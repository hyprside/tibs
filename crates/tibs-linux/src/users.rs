use color_eyre::Result;
use tibs_service_definitions::{UserAccount, UserId, UserRepositoryService};
use uzers::os::unix::UserExt;

pub struct LinuxUserRepository;

impl LinuxUserRepository {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxUserRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl UserRepositoryService for LinuxUserRepository {
    fn list_users(&self) -> Result<Vec<UserAccount>> {
        Ok(unsafe { uzers::all_users() }
            .filter(|user| {
                let uid = user.uid();
                uid >= 1000 && uid < 65534 && !user.shell().ends_with("nologin")
            })
            .map(|user| {
                let username = user.name().to_string_lossy().to_string();
                let home_dir = user.home_dir().to_path_buf();
                let avatar_path = Some(home_dir.join(".face")).filter(|path| path.exists());

                UserAccount {
                    id: UserId(user.uid().to_string()),
                    display_name: username.clone(),
                    username,
                    home_dir: Some(home_dir),
                    avatar_path,
                }
            })
            .collect())
    }
}
