//! Help → community and project links (the ArtCraft Discord, website, this app's page and its
//! GitHub repository). The commands return the URL; the frontend opens it.

use serde_json::{Value, json};

use super::{CommandSpec, Result, always, cmd};

/// The app's id on getartcraft.com and GitHub.
pub const APP_ID: &str = "vectorcraft";
pub const DISCORD_URL: &str = "https://discord.gg/5jvBRNw27j";
pub const WEBSITE_URL: &str = "https://www.rbhgrading.com";

/// This app's page on the website.
pub fn app_page_url() -> String {
    WEBSITE_URL.to_string()
}

/// This app's source repository.
pub fn github_url() -> String {
    "https://github.com/10Shilling6Pence/rbh-label-studio".to_string()
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "help.discord", "Join Our Discord", ["Help"], None, "{} → {url} the RBH community Discord", always, |_, _| url(DISCORD_URL.into())),
        cmd!(query "help.website", "RBH Grading Website", ["Help"], None, "{} → {url}", always, |_, _| url(WEBSITE_URL.into())),
        cmd!(query "help.appPage", "RBH Grading Website", ["Help"], None, "{} → {url} this app's page", always, |_, _| url(app_page_url())),
        cmd!(query "help.github", "RBH Label Studio on GitHub", ["Help"], None, "{} → {url} source code, issues and releases", always, |_, _| url(github_url())),
        cmd!(query "help.links", "Links", [], None, "{} → {discord, website, appPage, github}", always, |_, _| {
            Ok(json!({ "discord": DISCORD_URL, "website": WEBSITE_URL, "appPage": app_page_url(), "github": github_url() }))
        }),
    ]
}

fn url(u: String) -> Result<Value> {
    Ok(json!({ "url": u }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;

    #[test]
    fn links() {
        let mut s = Session::new();
        assert_eq!(s.execute("help.discord", &json!({})).unwrap()["url"], "https://discord.gg/5jvBRNw27j");
        assert_eq!(s.execute("help.appPage", &json!({})).unwrap()["url"], "https://www.rbhgrading.com");
        assert_eq!(s.execute("help.github", &json!({})).unwrap()["url"], "https://github.com/10Shilling6Pence/rbh-label-studio");
        assert_eq!(s.execute("help.links", &json!({})).unwrap()["website"], "https://www.rbhgrading.com");
    }
}
