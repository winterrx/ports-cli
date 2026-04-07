use crate::model::ListenerEntry;
use anyhow::{Result, bail};
use std::collections::BTreeMap;
use std::mem::size_of;
use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};

pub fn listening_tcp_ports() -> Result<Vec<ListenerEntry>> {
    let mut listeners = BTreeMap::new();

    for row in ipv4_rows()?.into_iter().chain(ipv6_rows()?) {
        let port = port_from_raw(row.0);
        if port == 0 {
            continue;
        }
        listeners
            .entry(port)
            .or_insert(ListenerEntry { port, pid: row.1 });
    }

    Ok(listeners.into_values().collect())
}

fn ipv4_rows() -> Result<Vec<(u32, u32)>> {
    let buffer = tcp_table(AF_INET as u32)?;
    let count = unsafe { *(buffer.as_ptr() as *const u32) as usize };
    let rows_ptr = unsafe { buffer.as_ptr().add(size_of::<u32>()) as *const MIB_TCPROW_OWNER_PID };
    let rows = unsafe { std::slice::from_raw_parts(rows_ptr, count) };
    Ok(rows
        .iter()
        .map(|row| (row.dwLocalPort, row.dwOwningPid))
        .collect())
}

fn ipv6_rows() -> Result<Vec<(u32, u32)>> {
    let buffer = tcp_table(AF_INET6 as u32)?;
    let count = unsafe { *(buffer.as_ptr() as *const u32) as usize };
    let rows_ptr = unsafe { buffer.as_ptr().add(size_of::<u32>()) as *const MIB_TCP6ROW_OWNER_PID };
    let rows = unsafe { std::slice::from_raw_parts(rows_ptr, count) };
    Ok(rows
        .iter()
        .map(|row| (row.dwLocalPort, row.dwOwningPid))
        .collect())
}

fn tcp_table(address_family: u32) -> Result<Vec<u8>> {
    let mut size = 0;
    let initial = unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut size,
            0,
            address_family,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };

    if initial != ERROR_INSUFFICIENT_BUFFER {
        bail!("GetExtendedTcpTable sizing failed with code {initial}");
    }

    let mut buffer = vec![0u8; size as usize];
    let result = unsafe {
        GetExtendedTcpTable(
            buffer.as_mut_ptr() as *mut _,
            &mut size,
            0,
            address_family,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };

    if result != NO_ERROR {
        bail!("GetExtendedTcpTable failed with code {result}");
    }

    if buffer.len() < size_of::<u32>() {
        bail!("GetExtendedTcpTable returned an undersized buffer")
    }

    Ok(buffer)
}

fn port_from_raw(raw: u32) -> u16 {
    u16::from_be(raw as u16)
}
