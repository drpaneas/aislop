use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::Deserialize;

// --- Data types from the GitHub API ---

#[derive(Debug, Deserialize)]
pub struct PrData {
    pub title: String,
    pub body: Option<String>,
    pub labels: Vec<Label>,
    pub user: User,
}

#[derive(Debug, Deserialize)]
pub struct Label {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub login: String,
}

#[derive(Debug, Deserialize)]
pub struct CommitData {
    pub sha: String,
    pub commit: CommitDetail,
}

#[derive(Debug, Deserialize)]
pub struct CommitDetail {
    pub message: String,
    pub author: GitUser,
    pub committer: GitUser,
}

#[derive(Debug, Deserialize)]
pub struct GitUser {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct IssueComment {
    pub id: u64,
    pub body: Option<String>,
}

// --- PR URL parsing ---

pub struct PrRef {
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

pub fn parse_pr_url(url: &str) -> Result<PrRef> {
    let url = url.trim().trim_end_matches('/');
    let parts: Vec<&str> = url.split('/').collect();

    // Expected: https://github.com/owner/repo/pull/123
    //           [0]     [1] [2]        [3]   [4]  [5] [6]
    if parts.len() < 7 || parts[2] != "github.com" || parts[5] != "pull" {
        bail!(
            "invalid PR URL format\n\
             expected: https://github.com/owner/repo/pull/123\n\
             got:      {url}"
        );
    }

    let number: u64 = parts[6]
        .parse()
        .context(format!("invalid PR number in URL: {}", parts[6]))?;

    Ok(PrRef {
        owner: parts[3].to_string(),
        repo: parts[4].to_string(),
        number,
    })
}

// --- GitHub API client ---

pub struct GitHubClient {
    client: Client,
    token: String,
    base_url: String,
}

impl GitHubClient {
    pub fn new(token: String) -> Result<Self> {
        let client = Client::builder()
            .user_agent("aislop/0.1.0")
            .build()
            .context("failed to create HTTP client")?;

        Ok(Self {
            client,
            token,
            base_url: "https://api.github.com".to_string(),
        })
    }

    pub async fn fetch_pr(&self, pr: &PrRef) -> Result<PrData> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.base_url, pr.owner, pr.repo, pr.number
        );

        let resp = self.get(&url, "application/vnd.github.v3+json").await?;
        resp.json::<PrData>()
            .await
            .context("failed to parse PR response")
    }

    pub async fn fetch_commits(&self, pr: &PrRef) -> Result<Vec<CommitData>> {
        let mut all_commits = Vec::new();
        let mut page: u32 = 1;

        loop {
            let url = format!(
                "{}/repos/{}/{}/pulls/{}/commits?per_page=100&page={page}",
                self.base_url, pr.owner, pr.repo, pr.number
            );

            let resp = self.get(&url, "application/vnd.github.v3+json").await?;
            let commits: Vec<CommitData> = resp
                .json()
                .await
                .context("failed to parse commits response")?;

            let count = commits.len();
            all_commits.extend(commits);

            if count < 100 {
                break;
            }
            page += 1;
        }

        Ok(all_commits)
    }

    pub async fn fetch_diff(&self, pr: &PrRef) -> Result<String> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.base_url, pr.owner, pr.repo, pr.number
        );

        let resp = self.get(&url, "application/vnd.github.v3.diff").await?;
        resp.text().await.context("failed to read diff body")
    }

    pub async fn post_comment(&self, pr: &PrRef, body: &str) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}/comments",
            self.base_url, pr.owner, pr.repo, pr.number
        );

        let payload = serde_json::json!({ "body": body });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .json(&payload)
            .send()
            .await
            .context("failed to post comment to PR")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("GitHub API error posting comment ({status}): {text}");
        }

        Ok(())
    }

    pub async fn find_bot_comment(&self, pr: &PrRef) -> Result<Option<u64>> {
        let mut page: u32 = 1;

        loop {
            let url = format!(
                "{}/repos/{}/{}/issues/{}/comments?per_page=100&page={page}",
                self.base_url, pr.owner, pr.repo, pr.number
            );

            let resp = self.get(&url, "application/vnd.github.v3+json").await?;
            let comments: Vec<IssueComment> = resp
                .json()
                .await
                .context("failed to parse comments response")?;

            let count = comments.len();

            for c in &comments {
                if let Some(body) = &c.body {
                    if body.starts_with("## aislop:") {
                        return Ok(Some(c.id));
                    }
                }
            }

            if count < 100 {
                break;
            }
            page += 1;
        }

        Ok(None)
    }

    pub async fn update_comment(&self, pr: &PrRef, comment_id: u64, body: &str) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/issues/comments/{comment_id}",
            self.base_url, pr.owner, pr.repo
        );

        let payload = serde_json::json!({ "body": body });

        let resp = self
            .client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .json(&payload)
            .send()
            .await
            .context("failed to update comment on PR")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("GitHub API error updating comment ({status}): {text}");
        }

        Ok(())
    }

    async fn get(&self, url: &str, accept: &str) -> Result<reqwest::Response> {
        let resp = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", accept)
            .send()
            .await
            .context("GitHub API request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("GitHub API error ({status}): {text}");
        }

        Ok(resp)
    }
}
