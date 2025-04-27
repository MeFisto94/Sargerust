use framexml_parser::toc::TocFile;

#[test]
fn toc_wowpedia() {
    // Source (was) https://wowpedia.fandom.com/wiki/TOC_format
    let toc = r#"## Interface: 110005
## Title: Waiting for Godot
## Notes: Nothing to be done.
## Version: 1.0.0

# This line is a comment
Vladimir.xml
Estragon.lua
libs\SomeLibrary.lua#"#;

    let parsed_toc = TocFile::parse_file(toc.as_bytes()).unwrap();
    assert_eq!(parsed_toc.directives.len(), 4);
    assert_eq!(
        parsed_toc.directives.get("Interface"),
        Some(&"110005".to_string())
    );
    assert_eq!(
        parsed_toc.directives.get("Title"),
        Some(&"Waiting for Godot".to_string())
    );
    assert_eq!(
        parsed_toc.directives.get("Notes"),
        Some(&"Nothing to be done.".to_string())
    );
    assert_eq!(
        parsed_toc.directives.get("Version"),
        Some(&"1.0.0".to_string())
    );
    assert_eq!(parsed_toc.files.len(), 3);
    assert_eq!(parsed_toc.files[0], "Vladimir.xml");
    assert_eq!(parsed_toc.files[1], "Estragon.lua");
    assert_eq!(parsed_toc.files[2], "libs\\SomeLibrary.lua");
    assert_eq!(parsed_toc.comments.len(), 1);
    assert_eq!(parsed_toc.comments[0], "This line is a comment");
}

#[test]
fn toc_blizz_comments() {
    let toc = "## This is a comment";
    let parsed_toc = TocFile::parse_file(toc.as_bytes()).unwrap();
    assert_eq!(parsed_toc.directives.len(), 0);
    assert_eq!(parsed_toc.files.len(), 0);
    assert_eq!(parsed_toc.comments.len(), 1);
    assert_eq!(parsed_toc.comments[0], "# This is a comment");
}
