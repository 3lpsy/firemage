use anyhow::{Result, ensure};
use thirtyfour::prelude::*;

pub async fn keyboard_toggle(input: &WebElement) -> Result<()> {
    ensure!(input.attr("role").await?.as_deref() == Some("switch"));
    let was_selected = input.is_selected().await?;
    input.send_keys(" ").await?;
    ensure!(
        input.is_selected().await? != was_selected,
        "Space did not toggle the switch"
    );
    Ok(())
}
