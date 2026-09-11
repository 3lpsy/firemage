use crate::{Harness, PASSWORD};
use anyhow::{Context, Result};
use std::time::Duration;
use thirtyfour::prelude::*;

impl Harness {
    pub async fn element(&self, by: By) -> Result<WebElement> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            if let Ok(element) = self.driver.find(by.clone()).await
                && element.is_displayed().await.unwrap_or(false)
                && element.is_enabled().await.unwrap_or(false)
            {
                return Ok(element);
            }
            anyhow::ensure!(
                tokio::time::Instant::now() < deadline,
                "element did not become visible: {by:?}"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    pub async fn button(&self, label: &str) -> Result<()> {
        self.element(By::XPath(format!("//button[normalize-space(.)='{label}']")))
            .await?
            .click()
            .await?;
        Ok(())
    }
    pub async fn modal_button(&self, label: &str) -> Result<()> {
        self.element(By::XPath(format!(
            "//*[@role='dialog']//button[normalize-space(.)='{label}']"
        )))
        .await?
        .click()
        .await?;
        Ok(())
    }
    pub async fn fill(&self, id: &str, value: &str) -> Result<()> {
        let element = self.element(By::Id(id)).await?;
        element.click().await?;
        element.send_keys(Key::Control + "a").await?;
        element.send_keys(Key::Backspace).await?;
        if !value.is_empty() {
            element.send_keys(value).await?;
        }
        Ok(())
    }
    pub async fn value(&self, id: &str) -> Result<String> {
        self.element(By::Id(id))
            .await?
            .prop("value")
            .await?
            .context("input has no value")
    }
    // Options may arrive after the select renders. SelectElement silently succeeds on no match.
    pub async fn select_value(&self, id: &str, value: &str) -> Result<()> {
        let select = self.element(By::Id(id)).await?;
        select.scroll_into_view().await?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            for option in select.find_all(By::Tag("option")).await? {
                if option.prop("value").await?.as_deref() == Some(value) {
                    option.click().await?;
                    anyhow::ensure!(
                        select.prop("value").await?.as_deref() == Some(value),
                        "select {id} did not retain option {value}"
                    );
                    return Ok(());
                }
            }
            anyhow::ensure!(
                tokio::time::Instant::now() < deadline,
                "option {value} did not load in select {id}"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    pub async fn navigate(&self, page: &str) -> Result<()> {
        self.element(By::XPath(format!(
            "//aside[contains(@class,'sidebar')]//button[contains(normalize-space(.),'{page}')]"
        )))
        .await?
        .click()
        .await?;
        self.element(By::XPath(format!(
            "//main//h1[normalize-space(.)='{page}']"
        )))
        .await?;
        Ok(())
    }
    pub async fn text(&self, text: &str) -> Result<()> {
        self.element(By::XPath(format!(
            "//*[not(self::script) and not(self::style) and contains(text(),'{text}')]"
        )))
        .await?;
        Ok(())
    }
    pub async fn absent(&self, by: By) -> Result<()> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            if self.driver.find_all(by.clone()).await?.is_empty() {
                return Ok(());
            }
            anyhow::ensure!(
                tokio::time::Instant::now() < deadline,
                "element remained present: {by:?}"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    pub async fn login(&self, username: &str) -> Result<()> {
        self.fill("login-username", username).await?;
        self.fill("login-password", PASSWORD).await?;
        self.button("Sign in").await?;
        self.element(By::Css(".app-shell")).await?;
        Ok(())
    }
    pub async fn logout(&self) -> Result<()> {
        self.element(By::Css("button[aria-label='Sign out']"))
            .await?
            .click()
            .await?;
        self.element(By::Id("login-username")).await?;
        Ok(())
    }
    pub async fn row_button(&self, text: &str, button: &str) -> Result<()> {
        self.element(By::XPath(format!(
            "//tr[.//*[normalize-space(.)='{text}']]//button[normalize-space(.)='{button}']"
        )))
        .await?
        .click()
        .await?;
        Ok(())
    }
    pub async fn radio(&self, name: &str, label: &str) -> Result<()> {
        self.element(By::XPath(format!("//label[contains(normalize-space(.),'{label}')]/input[@type='radio' and @name='{name}']"))).await?.click().await?;
        Ok(())
    }
    pub async fn select_kernel(&self, name: &str) -> Result<()> {
        self.element(By::Id("vm-kernel")).await?.click().await?;
        self.fill("vm-kernel-search", name).await?;
        self.element(By::XPath(format!(
            "//*[@role='option'][.//*[normalize-space(.)='{name}']]"
        )))
        .await?
        .click()
        .await?;
        self.absent(By::Id("vm-kernel-search")).await
    }
    pub async fn screenshot(&self, stage: &str) -> Result<()> {
        let path = self.screenshots.join(format!("{}-{stage}.png", self.name));
        self.driver.screenshot(&path).await?;
        println!("Browser screenshot: {}", path.display());
        Ok(())
    }
}
