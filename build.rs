use vergen_gitcl::{Emitter, GitclBuilder};

fn main() -> anyhow::Result<()> {
    let git = GitclBuilder::default().sha(true).branch(true).build()?;

    Emitter::default().add_instructions(&git)?.emit()?;

    Ok(())
}
