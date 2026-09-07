use color_eyre::Result;
use std::path::PathBuf;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct UserId(pub String);

#[derive(Clone, Debug)]
pub struct UserAccount {
    pub id: UserId,
    pub username: String,
    pub display_name: String,
    pub home_dir: Option<PathBuf>,
    pub avatar_path: Option<PathBuf>,
}

pub trait UserRepositoryService {
    fn list_users(&self) -> Result<Vec<UserAccount>>;
}
