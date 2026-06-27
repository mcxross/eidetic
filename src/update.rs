use self_update::cargo_crate_version;

pub async fn update() -> anyhow::Result<()> {
    tokio::task::spawn_blocking(|| {
        println!("Checking for updates...");

        let status = self_update::backends::github::Update::configure()
            .repo_owner("mcxross")
            .repo_name("eidetic")
            .bin_name("eidetic")
            .show_download_progress(true)
            .current_version(cargo_crate_version!())
            .build()?
            .update()?;

        println!("Update status: `{}`!", status.version());
        Ok::<(), anyhow::Error>(())
    })
    .await??;
    Ok(())
}
