use color_eyre::Result;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct DesktopSessionId(pub String);

#[derive(Clone, Debug)]
pub struct DesktopSession {
    pub id: DesktopSessionId,
    pub name: String,
    pub command: String,
}

pub trait DesktopSessionRepositoryService {
    fn list_sessions(&self) -> Result<Vec<DesktopSession>>;
}
