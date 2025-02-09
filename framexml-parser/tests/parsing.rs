use framexml_parser::typedefs::Ui;
use quick_xml::de::Deserializer;
use serde::Deserialize;

#[test]
fn parse_foo() {
    let raid_warning_xml = include_str!("RaidWarning.xml");
    let mut deserializer = Deserializer::from_str(raid_warning_xml);

    let ui = Ui::deserialize(&mut deserializer).unwrap();
    panic!("{:?}", ui);
}
