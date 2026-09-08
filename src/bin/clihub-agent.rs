#[path = "../collab/store.rs"]
mod store;
#[path = "../collab/mcp.rs"]
mod mcp;
use anyhow::{anyhow, Result};
use std::{
    env, fs,
    io::{self, IsTerminal},
};
use store::*;
fn val(args: &[String], k: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == k).map(|w| w[1].clone())
}
fn main() -> Result<()> {
    let a: Vec<String> = env::args().collect();
    if a.len() == 1 || a.iter().any(|x| x == "--help" || x == "-h") {
        println!("CLIHub Agent Mailbox & MCP Server\n\nOpen clihub.exe for the Agent Chat window.\n\nCommands:\n  mcp --room DIR --agent ID\n  list --room DIR\n  inbox --room DIR --agent ID [--unread]\n  send --room DIR --from ID --to ID --body-file FILE [--reply-to ID]\n  read --room DIR --agent ID --message ID\n\nGet the room directory from Agent Chat in CLIHub.");
        if a.len() == 1 && io::stdin().is_terminal() {
            println!("\nPress Enter to close.");
            let mut s = String::new();
            io::stdin().read_line(&mut s)?;
        }
        return Ok(());
    }
    let room = val(&a, "--room").ok_or_else(|| anyhow!("--room DIR required"))?;
    let s = Store::new(&room)?;
    match a.get(1).map(String::as_str) {
        Some("mcp") => {
            let agent = val(&a, "--agent").ok_or_else(|| anyhow!("--agent required"))?;
            mcp::run_mcp_server(io::stdin().lock(), io::stdout().lock(), room, &agent)?;
        }
        Some("list") => println!("{}", serde_json::to_string_pretty(&s.load_round()?)?),
        Some("inbox") => {
            for m in s.messages(
                &val(&a, "--agent").ok_or_else(|| anyhow!("--agent required"))?,
                a.iter().any(|x| x == "--unread"),
            )? {
                println!("{}", serde_json::to_string(&m)?)
            }
        }
        Some("read") => {
            let id = val(&a, "--message").ok_or_else(|| anyhow!("--message required"))?;
            let m = s.read(&id)?;
            println!("{}", serde_json::to_string_pretty(&m)?);
            if let Some(agent) = val(&a, "--agent") {
                s.mark_read(&agent, &id)?;
            }
        }
        Some("send") => {
            let body = if let Some(b) = val(&a, "--body") {
                b
            } else if let Some(p) = val(&a, "--body-file") {
                fs::read_to_string(p)?
            } else {
                return Err(anyhow!("--body or --body-file required"));
            };
            let r = s.load_round()?;
            let m = Message {
                id: Store::new_id("m"),
                round: r.id,
                from: val(&a, "--from").ok_or_else(|| anyhow!("--from required"))?,
                to: val(&a, "--to").ok_or_else(|| anyhow!("--to required"))?,
                body,
                reply_to: val(&a, "--reply-to"),
                anchor: val(&a, "--anchor-file").map(|f| Anchor {
                    file: f.into(),
                    line: val(&a, "--anchor-line")
                        .and_then(|x| x.parse().ok())
                        .unwrap_or(0),
                    snapshot: String::new(),
                }),
                snapshot: val(&a, "--snapshot"),
                time: Store::now(),
            };
            let id = m.id.clone();
            s.send(m)?;
            println!("{}", id)
        }
        _ => return Err(anyhow!("commands: list, inbox, send, read")),
    };
    Ok(())
}
