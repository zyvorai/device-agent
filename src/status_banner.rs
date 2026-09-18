// SPDX-License-Identifier: Apache-2.0

//! Cilium-style colorful status for `zyvor-device-agent status`.

use crate::config::Config;

const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const GREEN: &str = "\x1b[32m";
const MAGENTA: &str = "\x1b[35m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

pub fn format_status(cfg: &Config) -> String {
    let tls = if cfg.server.tls.enabled {
        ok()
    } else {
        disabled()
    };
    let nodra = if cfg.nodra.enabled { ok() } else { disabled() };
    let fleet = if cfg.fleet.enabled { ok() } else { disabled() };
    let plugins = ok();
    let api = ok();

    let mut out = String::new();
    out.push_str(&format!("{YELLOW} /¯¯\\\n"));
    out.push_str(&format!(
        "{CYAN} /¯¯{YELLOW}\\__/{GREEN}¯¯\\{RESET}\tAPI:\t{api}\n"
    ));
    out.push_str(&format!(
        "{CYAN} \\__{RED}/¯¯\\{GREEN}__/{RESET}\tTLS:\t{tls}\n"
    ));
    out.push_str(&format!(
        "{GREEN} /¯¯{RED}\\__/{MAGENTA}¯¯\\{RESET}\tPlugins:\t{plugins}\n"
    ));
    out.push_str(&format!(
        "{GREEN} \\__{BLUE}/¯¯\\{MAGENTA}__/{RESET}\tNodra:\t{nodra}\n"
    ));
    out.push_str(&format!(
        "{BLUE}{BLUE}{BLUE} \\__/{RESET}\tFleet:\t{fleet}\n\n"
    ));
    out.push_str(&format!("🖥️  Listen:    {}\n", cfg.server.listen));
    out.push_str("✨ Features\n");
    for (name, on) in [
        ("Inventory", true),
        ("Doctor", true),
        ("Sensors", true),
        ("Industrial", true),
        ("Enrollment", !cfg.enrollment.server_url.is_empty()),
        ("Nodra MQTT", cfg.nodra.enabled),
        ("Fleet", cfg.fleet.enabled),
        ("TLS", cfg.server.tls.enabled),
    ] {
        let state = if on { ok() } else { disabled() };
        out.push_str(&format!("               {name:<16} {state}\n"));
    }
    out
}

fn ok() -> String {
    format!("✅ {GREEN}OK{RESET}")
}
fn disabled() -> String {
    format!("ℹ️  {CYAN}disabled{RESET}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_mentions_components() {
        let text = format_status(&Config::default());
        assert!(text.contains("/¯¯\\"));
        assert!(text.contains("API:"));
        assert!(text.contains("Nodra:"));
        assert!(text.contains("✨ Features"));
        assert!(text.contains("OK") || text.contains("disabled"));
    }
}
