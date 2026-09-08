use zbus_systemd::{login1, zbus::Connection};

type M = color_eyre::Result<()>;
async fn main_async() -> M {
    let connection = Connection::system().await?;
    let login = login1::ManagerProxy::new(&connection).await?;
    for (id, uid, user, seat, path) in login.list_sessions().await? {
        println!("{id}: {user} (uid={uid}), seat={seat}, path={path}");
    }
    Ok(())
}

fn main() -> M {
    color_eyre::install()?;
    smol::block_on(main_async())
}
