//! Explain a specific finding

use anyhow::Result;

pub fn run(finding_id: &str, use_ai: bool) -> Result<()> {
    println!("Explaining finding: {}", finding_id);

    if use_ai {
        println!("(AI explanation would be generated here)");
    } else {
        println!("(Detailed explanation would be shown here)");
    }

    Ok(())
}
