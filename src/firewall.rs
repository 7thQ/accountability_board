use std::io::Write;
use std::process::{Command, Stdio};

const PORT_RULE: &str = "3000/tcp";

pub fn prompt_password() -> Option<String> {
    println!("Opening port 3000 in the firewall (ufw) so phones on the LAN can connect.");
    rpassword::prompt_password("[sudo] password: ").ok()
}

fn run_ufw(password: &str, args: &[&str]) -> bool {
    let Ok(mut child) = Command::new("sudo")
        .arg("-S")
        .arg("-p")
        .arg("")
        .arg("ufw")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };

    if let Some(stdin) = child.stdin.as_mut() {
        let _ = writeln!(stdin, "{password}");
    }

    child.wait().map(|status| status.success()).unwrap_or(false)
}

pub fn allow_port(password: &str) -> bool {
    let ok = run_ufw(password, &["allow", PORT_RULE]);
    if ok {
        println!("Firewall rule added: {PORT_RULE} is now open.");
    } else {
        println!(
            "Could not add the firewall rule — phones on the LAN may not be able to connect. \
             You can run `sudo ufw allow {PORT_RULE}` yourself."
        );
    }
    ok
}

pub fn revert_port(password: &str) {
    if run_ufw(password, &["delete", "allow", PORT_RULE]) {
        println!("Firewall rule removed: {PORT_RULE} is closed again.");
    } else {
        println!(
            "Could not automatically remove the firewall rule — run \
             `sudo ufw delete allow {PORT_RULE}` yourself if you want it closed."
        );
    }
}
