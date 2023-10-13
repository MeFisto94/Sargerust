/// Contrasting to the importers, that convert already parsed files into our rendering IR,
/// Loaders are a lot more high level. They call the parsers and pipe them into importers
pub mod blp_loader;
pub mod m2_loader;
pub mod wmo_loader;
