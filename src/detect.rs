use crate::model::ProcessSnapshot;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const PROJECT_MARKERS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "go.mod",
    "pyproject.toml",
    "Gemfile",
    "pom.xml",
    "build.gradle",
    ".git",
];

pub fn is_dev_process(process_name: &str, command: &str) -> bool {
    let name = process_name.to_ascii_lowercase();
    let command = command.to_ascii_lowercase();

    let system_prefixes = [
        "svchost",
        "csrss",
        "lsass",
        "services",
        "explorer",
        "dwm",
        "searchindexer",
        "taskhostw",
        "runtimebroker",
        "shellexperiencehost",
        "spotify",
        "raycast",
        "postman",
        "slack",
        "discord",
        "firefox",
        "chrome",
        "google",
        "zoom",
        "teams",
        "code",
    ];
    if system_prefixes
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        return false;
    }

    let dev_names = [
        "node",
        "bun",
        "python",
        "python3",
        "ruby",
        "java",
        "go",
        "cargo",
        "deno",
        "php",
        "uvicorn",
        "gunicorn",
        "flask",
        "rails",
        "npm",
        "npx",
        "yarn",
        "pnpm",
        "tsx",
        "esbuild",
        "rollup",
        "turbo",
        "nx",
        "jest",
        "vitest",
        "pytest",
        "docker",
        "docker desktop",
        "com.docker.backend",
        "rustc",
        "dotnet",
    ];
    if dev_names.iter().any(|candidate| name == *candidate) {
        return true;
    }

    if name.contains("docker") {
        return true;
    }

    let indicators = [
        " next",
        "vite",
        "nuxt",
        "webpack",
        "remix",
        "astro",
        "django",
        "manage.py",
        "uvicorn",
        "rails",
        "cargo",
        "tsx",
        "node",
        "bun",
        "python",
    ];

    indicators
        .iter()
        .any(|indicator| command.contains(indicator))
}

pub fn is_dockerish(process_name: &str, command: &str) -> bool {
    let name = process_name.to_ascii_lowercase();
    let command = command.to_ascii_lowercase();
    name.contains("docker") || command.contains("docker")
}

pub fn is_noise_process(process_name: &str, command: &str) -> bool {
    let name = process_name.to_ascii_lowercase();
    let command = command.to_ascii_lowercase();

    let wrapper_names = ["cmd.exe", "cmd", "bash.exe", "bash", "wsl.exe", "wsl"];
    if wrapper_names.iter().any(|candidate| name == *candidate) {
        return true;
    }

    let helper_names = ["rustc.exe", "rustc", "uv.exe", "uv"];
    if helper_names.iter().any(|candidate| name == *candidate) {
        return true;
    }

    let helper_patterns = [
        "tsserver.js",
        "typingsinstaller.js",
        "eslintserver.js",
        "tailwindcss-language-server",
        "yaml-language-server",
        "chromedevtools-mcp.js",
        "chrome-devtools-mcp.js",
        "chrome-devtools",
        "processchild.js",
        "child.js",
        "vtsls.js",
        "partialsemantic",
        "tscancellation",
        "gsc_server.py",
    ];

    helper_patterns
        .iter()
        .any(|pattern| command.contains(pattern))
}

pub fn detect_framework_from_image(image: &str) -> String {
    let image = image.to_ascii_lowercase();
    if image.contains("postgres") {
        "PostgreSQL"
    } else if image.contains("redis") {
        "Redis"
    } else if image.contains("mysql") || image.contains("mariadb") {
        "MySQL"
    } else if image.contains("mongo") {
        "MongoDB"
    } else if image.contains("nginx") {
        "nginx"
    } else if image.contains("localstack") {
        "LocalStack"
    } else if image.contains("rabbitmq") {
        "RabbitMQ"
    } else if image.contains("kafka") {
        "Kafka"
    } else if image.contains("elasticsearch") || image.contains("opensearch") {
        "Elasticsearch"
    } else if image.contains("minio") {
        "MinIO"
    } else {
        "Docker"
    }
    .to_string()
}

pub fn detect_framework_from_command(command: &str, process_name: &str) -> Option<String> {
    let command = command.to_ascii_lowercase();
    let name = process_name.to_ascii_lowercase();

    let framework = if command.contains("next") {
        Some("Next.js")
    } else if command.contains("bun") || name == "bun" || name == "bun.exe" {
        Some("Bun")
    } else if command.contains("vite") {
        Some("Vite")
    } else if command.contains("nuxt") {
        Some("Nuxt")
    } else if command.contains("angular") || command.contains("ng serve") {
        Some("Angular")
    } else if command.contains("webpack") {
        Some("Webpack")
    } else if command.contains("remix") {
        Some("Remix")
    } else if command.contains("astro") {
        Some("Astro")
    } else if command.contains("flask") {
        Some("Flask")
    } else if command.contains("django") || command.contains("manage.py") {
        Some("Django")
    } else if command.contains("uvicorn") || command.contains("fastapi") {
        Some("FastAPI")
    } else if command.contains("rails") {
        Some("Rails")
    } else if command.contains("cargo") || command.contains("rustc") {
        Some("Rust")
    } else if name == "node" {
        Some("Node.js")
    } else if name == "python" || name == "python3" {
        Some("Python")
    } else if name == "ruby" {
        Some("Ruby")
    } else if name == "java" {
        Some("Java")
    } else if name == "go" {
        Some("Go")
    } else {
        None
    };

    framework.map(ToString::to_string)
}

pub fn detect_framework_from_project(project_root: &Path) -> Option<String> {
    let package_json = project_root.join("package.json");
    let has_package_json = package_json.exists();
    if let Ok(contents) = fs::read_to_string(&package_json) {
        if let Ok(value) = serde_json::from_str::<Value>(&contents) {
            let dependencies = value
                .get("dependencies")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(|deps| deps.keys().map(String::as_str));
            let dev_dependencies = value
                .get("devDependencies")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(|deps| deps.keys().map(String::as_str));
            let keys = dependencies.chain(dev_dependencies).collect::<Vec<_>>();

            let framework = if keys.contains(&"next") {
                Some("Next.js")
            } else if keys.contains(&"nuxt") || keys.contains(&"nuxt3") {
                Some("Nuxt")
            } else if keys.contains(&"@sveltejs/kit") {
                Some("SvelteKit")
            } else if keys.contains(&"svelte") {
                Some("Svelte")
            } else if keys.contains(&"@remix-run/react") || keys.contains(&"remix") {
                Some("Remix")
            } else if keys.contains(&"astro") {
                Some("Astro")
            } else if keys.contains(&"vite") {
                Some("Vite")
            } else if keys.contains(&"@angular/core") {
                Some("Angular")
            } else if keys.contains(&"vue") {
                Some("Vue")
            } else if keys.contains(&"react") {
                Some("React")
            } else if keys.contains(&"express") {
                Some("Express")
            } else if keys.contains(&"fastify") {
                Some("Fastify")
            } else if keys.contains(&"hono") {
                Some("Hono")
            } else if keys.contains(&"koa") {
                Some("Koa")
            } else if keys.contains(&"@nestjs/core") {
                Some("NestJS")
            } else {
                None
            };
            if let Some(framework) = framework {
                return Some(framework.to_string());
            }
        }
    }

    if has_package_json {
        return None;
    }

    let inferred = if project_root.join("vite.config.ts").exists()
        || project_root.join("vite.config.js").exists()
    {
        Some("Vite")
    } else if project_root.join("next.config.js").exists()
        || project_root.join("next.config.mjs").exists()
    {
        Some("Next.js")
    } else if project_root.join("angular.json").exists() {
        Some("Angular")
    } else if project_root.join("Cargo.toml").exists() {
        Some("Rust")
    } else if project_root.join("go.mod").exists() {
        Some("Go")
    } else if project_root.join("manage.py").exists() {
        Some("Django")
    } else if project_root.join("Gemfile").exists() {
        Some("Ruby")
    } else {
        None
    };

    inferred.map(ToString::to_string)
}

pub fn project_root_from_snapshot_cached(
    snapshot: &ProcessSnapshot,
    root_cache: &mut HashMap<PathBuf, Option<PathBuf>>,
) -> Option<PathBuf> {
    if let Some(cwd) = snapshot.cwd.as_ref()
        && let Some(root) = find_project_root_cached(cwd, root_cache)
    {
        return Some(root);
    }

    if let Some(exe_dir) = snapshot.exe.as_ref().and_then(|path| path.parent())
        && let Some(root) = find_project_root_cached(exe_dir, root_cache)
    {
        return Some(root);
    }

    for token in snapshot.command.split_whitespace() {
        let cleaned = token.trim_matches('"');
        if cleaned.starts_with('-') || cleaned.len() < 3 || !looks_like_path_token(cleaned) {
            continue;
        }
        let path = Path::new(cleaned);
        if path.exists() {
            let candidate = if path.is_dir() {
                path.to_path_buf()
            } else {
                path.parent().unwrap_or(path).to_path_buf()
            };
            if let Some(root) = find_project_root_cached(&candidate, root_cache) {
                return Some(root);
            }
        }
    }

    None
}

pub fn summarize_command(command: &str, process_name: &str) -> String {
    let parts = command
        .split_whitespace()
        .skip(1)
        .filter(|part| !part.starts_with('-'))
        .map(|part| part.trim_matches('"'))
        .filter(|part| !part.is_empty())
        .take(3)
        .map(short_name)
        .collect::<Vec<_>>();

    if parts.is_empty() {
        process_name.to_string()
    } else {
        parts.join(" ")
    }
}

fn find_project_root_cached(
    path: &Path,
    root_cache: &mut HashMap<PathBuf, Option<PathBuf>>,
) -> Option<PathBuf> {
    if let Some(cached) = root_cache.get(path) {
        return cached.clone();
    }

    let mut current = path.to_path_buf();
    let mut traversed = Vec::new();
    for _ in 0..15 {
        traversed.push(current.clone());
        if PROJECT_MARKERS
            .iter()
            .any(|marker| current.join(marker).exists())
        {
            let found = Some(current);
            for candidate in traversed {
                root_cache.insert(candidate, found.clone());
            }
            return found;
        }
        if !current.pop() {
            break;
        }
    }

    for candidate in traversed {
        root_cache.insert(candidate, None);
    }

    None
}

fn looks_like_path_token(token: &str) -> bool {
    token.contains(['\\', '/', ':', '.'])
}

fn short_name(value: &str) -> String {
    Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(value)
        .to_string()
}
