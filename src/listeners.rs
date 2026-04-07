use crate::model::ListenerEntry;
use anyhow::Result;

pub fn listening_tcp_ports() -> Result<Vec<ListenerEntry>> {
    #[cfg(windows)]
    {
        return crate::windows_tcp::listening_tcp_ports();
    }

    #[cfg(unix)]
    {
        return crate::unix_tcp::listening_tcp_ports();
    }

    #[cfg(not(any(windows, unix)))]
    {
        bail!("listening port lookup is not implemented for this platform")
    }
}
