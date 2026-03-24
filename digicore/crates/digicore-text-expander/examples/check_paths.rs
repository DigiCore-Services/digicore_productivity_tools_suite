use digicore_text_expander::ports::data_path_resolver::DataPathResolver;

fn main() {
    println!("DB PATH: {:?}", DataPathResolver::db_path());
    println!("CONFIG DIR: {:?}", dirs::config_dir().unwrap().join("com.digicore.text-expander"));
}
