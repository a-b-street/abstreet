fn main() {
    let popdat = popdat::PopDat::import_all(&mut abstutil::Timer::new("importing popdat"));
    abstutil::write_binary("../data/shapes/popdat", &popdat).unwrap();
}
