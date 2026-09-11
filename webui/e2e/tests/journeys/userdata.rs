use anyhow::{Result, ensure};
use firemage_webui_e2e::Harness;
use serde_json::json;
use thirtyfour::prelude::*;

pub async fn editor(h: &Harness, id: &str) -> Result<()> {
    let script = format!(
        "#!/bin/sh\ncat <<'EOF'\n<img src=x onerror=alert(1)>\nrésumé \"literal\nEOF\n{}\n{}",
        "# extra line\n".repeat(24),
        "printf long-line ".repeat(25)
    );
    h.fill(id, &script).await?;
    ensure!(
        h.value(id).await? == script,
        "userdata editor changed pasted script bytes"
    );
    let state = h.driver.execute(
        "const input = document.getElementById(arguments[0]); const paint = input.parentElement.querySelector('.shell-editor-paint'); input.setSelectionRange(0, 9); return {text:paint.textContent, hidden:paint.getAttribute('aria-hidden'), elements:paint.querySelectorAll('img,script').length, selected:input.value.slice(input.selectionStart,input.selectionEnd)}",
        vec![json!(id)],
    ).await?;
    ensure!(
        state.json()["text"] == format!("{script}\n") && state.json()["elements"] == 0,
        "syntax painting parsed HTML or lost text: {:?}",
        state.json()
    );
    ensure!(
        state.json()["hidden"] == "true" && state.json()["selected"] == "#!/bin/sh",
        "textarea selection/accessibility changed"
    );
    h.driver.execute(
        "const input = document.getElementById(arguments[0]); input.scrollTop = 160; input.scrollLeft = 100; input.dispatchEvent(new Event('scroll'));",
        vec![json!(id)],
    ).await?;
    let scroll = h.driver.execute_async(
        "const id=arguments[0], done=arguments[arguments.length-1]; requestAnimationFrame(()=>requestAnimationFrame(()=>{const input=document.getElementById(id); const code=input.parentElement.querySelector('.shell-editor-paint code'); const matrix=new DOMMatrix(getComputedStyle(code).transform); done({left:input.scrollLeft,top:input.scrollTop,x:matrix.m41,y:matrix.m42});}));",
        vec![json!(id)],
    ).await?;
    let state = scroll.json();
    ensure!(
        state["top"].as_i64().unwrap_or_default() > 0
            && state["left"].as_i64().unwrap_or_default() > 0,
        "fixture did not scroll both axes: {state}"
    );
    ensure!(
        state["x"].as_f64() == state["left"].as_f64().map(|value| -value)
            && state["y"].as_f64() == state["top"].as_f64().map(|value| -value),
        "syntax paint did not follow textarea scrolling: {state}"
    );
    h.screenshot("userdata-highlighting").await?;
    Ok(())
}

pub async fn ownership(h: &Harness) -> Result<()> {
    let layout = h.driver.execute(
        "return [...document.querySelectorAll('.file-ownership-fields')].filter(row=>row.offsetWidth).map(row=>({overflow:row.scrollWidth>row.clientWidth,top:[...row.querySelectorAll('input')].map(input=>Math.round(input.getBoundingClientRect().top))}));",
        vec![],
    ).await?;
    let rows = layout.json().as_array().expect("ownership rows");
    ensure!(!rows.is_empty(), "no visible ownership controls");
    for row in rows {
        let top = row["top"].as_array().expect("ownership inputs");
        ensure!(
            row["overflow"] == false && top.len() == 3 && top.iter().all(|value| value == &top[0]),
            "ownership fields overflowed or wrapped: {row}"
        );
    }
    let remove = h
        .element(By::Css(
            ".destination-field-label button[aria-label='Remove boot file 1']",
        ))
        .await?;
    ensure!(
        remove.text().await?.trim().is_empty(),
        "boot removal is not icon-only"
    );
    Ok(())
}
