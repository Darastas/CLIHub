use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum Phase {
    Development,
    Review,
    Fix,
    Completed,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Participant {
    pub id: String,
    pub name: String,
    pub role: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Round {
    pub id: String,
    pub title: String,
    pub goal: String,
    pub repo: PathBuf,
    pub created: u64,
    pub phase: Phase,
    pub participants: Vec<Participant>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Anchor {
    pub file: PathBuf,
    pub line: u32,
    pub snapshot: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Message {
    pub id: String,
    pub round: String,
    pub from: String,
    pub to: String,
    pub body: String,
    pub reply_to: Option<String>,
    pub anchor: Option<Anchor>,
    pub snapshot: Option<String>,
    pub time: u64,
}
pub struct Store {
    pub room: PathBuf,
}
impl Store {
    pub fn new(room: impl Into<PathBuf>) -> Result<Self> {
        let room = room.into();
        fs::create_dir_all(room.join("messages"))?;
        fs::create_dir_all(room.join("reads"))?;
        Ok(Self { room })
    }
    pub fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
    pub fn new_id(prefix: &str) -> String {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!(
            "{}{}-{}-{}",
            prefix,
            n,
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        )
    }
    fn component(id: &str) -> Result<()> {
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            Err(anyhow!("invalid id"))
        } else {
            Ok(())
        }
    }
    pub fn create_round(&self, mut r: Round) -> Result<Round> {
        if r.id.is_empty() {
            r.id = Self::new_id("r");
        }
        Self::component(&r.id)?;
        if r.created == 0 {
            r.created = Self::now();
        }
        let p = self.room.join("round.json");
        if p.exists() {
            return Err(anyhow!("round already exists"));
        }
        Self::publish(&p, &serde_json::to_vec_pretty(&r)?)?;
        Ok(r)
    }
    pub fn load_round(&self) -> Result<Round> {
        Ok(serde_json::from_slice(&fs::read(
            self.room.join("round.json"),
        )?)?)
    }
    pub fn save_round(&self, r: &Round) -> Result<()> {
        let p = self.room.join("round.json");
        let t = p.with_extension(format!("tmp-{}", Self::new_id("w")));
        fs::write(&t, serde_json::to_vec_pretty(r)?)?;
        fs::rename(&t, p)?;
        Ok(())
    }
    pub fn send(&self, m: Message) -> Result<()> {
        Self::component(&m.id)?;
        if m.body.trim().is_empty() {
            return Err(anyhow!("empty body"));
        }
        let r = self.load_round()?;
        if m.round != r.id {
            return Err(anyhow!("round mismatch"));
        }
        let ids: Vec<_> = r.participants.iter().map(|p| p.id.as_str()).collect();
        if !ids.contains(&m.from.as_str()) || !ids.contains(&m.to.as_str()) {
            return Err(anyhow!("unknown participant"));
        }
        if let Some(id) = &m.reply_to {
            Self::component(id)?;
            if self.read(id)?.round != m.round {
                return Err(anyhow!("reply round mismatch"));
            }
        }
        if let Some(a) = &m.anchor {
            if a.line == 0 {
                return Err(anyhow!("anchor line must be positive"));
            }
        }
        let path = self.message_path(&m.id);
        Self::publish(&path, &serde_json::to_vec(&m)?)?;
        Ok(())
    }
    fn publish(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
        let tmp = path.with_extension(format!("tmp-{}", Self::new_id("write")));
        let result = fs::write(&tmp, bytes).and_then(|_| fs::hard_link(&tmp, path));
        let _ = fs::remove_file(tmp);
        result?;
        Ok(())
    }
    fn message_path(&self, id: &str) -> PathBuf {
        self.room.join("messages").join(format!("{}.json", id))
    }
    pub fn all_messages(&self) -> Result<Vec<Message>> {
        let mut out: Vec<Message> = vec![];
        for e in fs::read_dir(self.room.join("messages"))? {
            let p = e?.path();
            if p.extension().and_then(|x| x.to_str()) == Some("json") {
                out.push(serde_json::from_slice(&fs::read(p)?)?)
            }
        }
        out.sort_by(|a, b| (a.time, &a.id).cmp(&(b.time, &b.id)));
        Ok(out)
    }
    pub fn messages(&self, agent: &str, unread: bool) -> Result<Vec<Message>> {
        Ok(self
            .all_messages()?
            .into_iter()
            .filter(|m| m.to == agent && (!unread || !self.is_read(agent, &m.id)))
            .collect())
    }
    pub fn is_read(&self, a: &str, id: &str) -> bool {
        self.room
            .join("reads")
            .join(format!("{}-{}", a, id))
            .exists()
    }
    pub fn mark_read(&self, agent: &str, id: &str) -> Result<()> {
        Self::component(agent)?;
        Self::component(id)?;
        if self.read(id)?.to != agent {
            return Err(anyhow!("message recipient mismatch"));
        }
        fs::write(
            self.room.join("reads").join(format!("{}-{}", agent, id)),
            b"1",
        )?;
        Ok(())
    }
    pub fn read(&self, id: &str) -> Result<Message> {
        Self::component(id)?;
        Ok(serde_json::from_slice(&fs::read(self.message_path(id))?)?)
    }
}
