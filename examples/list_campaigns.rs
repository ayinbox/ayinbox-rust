//! Signs in and lists every campaign.
//! Run: AYINBOX_API_KEY=aybox_... cargo run --example list_campaigns

use ayinbox::{CampaignListParams, Client};
use futures_util::TryStreamExt;

#[tokio::main]
async fn main() -> Result<(), ayinbox::Error> {
    let client = Client::from_env()?;
    let me = client.me().get().await?;
    println!("Signed in as {}", me.email);

    let mut campaigns = client.campaigns().list_all(&CampaignListParams::default());
    while let Some(campaign) = campaigns.try_next().await? {
        println!("{}  {}  {}", campaign.id, campaign.status, campaign.name);
    }
    Ok(())
}
