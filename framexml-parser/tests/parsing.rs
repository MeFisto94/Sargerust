use framexml_parser::typedefs::Ui;
use quick_xml::de::Deserializer;
use serde::Deserialize;
use std::cmp::{max, min};

#[test]
fn parse_raid_warning() {
    let raid_warning_xml = include_str!("RaidWarning.xml");
    let mut deserializer = Deserializer::from_str(raid_warning_xml);

    let ui = Ui::deserialize(&mut deserializer).unwrap();
    panic!("Successfully deserialized to: {:?}", ui);
}

const CONTEXT: usize = 128;

#[test]
fn parse_account_login() {
    let account_login_xml = include_str!("Interface_GlueXML_AccountLogin.xml");
    let mut deserializer = Deserializer::from_str(account_login_xml);

    let ui = Ui::deserialize(&mut deserializer);
    if ui.is_err() {
        let line_number = deserializer.get_ref().get_ref().buffer_position() as usize;
        let lower_bound = max(0, line_number - CONTEXT);
        let upper_bound = min(line_number + CONTEXT, account_login_xml.len());
        let excerpt_before = &account_login_xml[lower_bound..line_number];
        let excerpt_after = &account_login_xml[line_number..upper_bound];
        panic!(
            "Failed to deserialize: {:?} @ {} \"{}\" <!-- ERROR HERE -->\"{}\"",
            ui.unwrap_err(),
            line_number,
            excerpt_before,
            excerpt_after
        );
    }
    panic!("Successfully deserialized to: {:?}", ui.unwrap());
}

#[test]
fn ui_parent() {
    let account_login_xml = include_str!("UIParent.xml");
    let mut deserializer = Deserializer::from_str(account_login_xml);

    let ui = Ui::deserialize(&mut deserializer);
    if ui.is_err() {
        let line_number = deserializer.get_ref().get_ref().buffer_position() as usize;
        let lower_bound = max(0, line_number - CONTEXT);
        let upper_bound = min(line_number + CONTEXT, account_login_xml.len());
        let excerpt_before = &account_login_xml[lower_bound..line_number];
        let excerpt_after = &account_login_xml[line_number..upper_bound];
        panic!(
            "Failed to deserialize: {:?} @ {} \"{}\" <!-- ERROR HERE -->\"{}\"",
            ui.unwrap_err(),
            line_number,
            excerpt_before,
            excerpt_after
        );
    }
    panic!("Successfully deserialized to: {:?}", ui.unwrap());
}
