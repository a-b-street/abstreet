fn main() {
    // TODO Productionize.
    let parcels = popdat::psrc::import_parcels(
        "/home/dabreegster/Downloads/psrc/2014/landuse/parcels_urbansim.txt",
    )
    .unwrap();
    println!("{} matches", parcels.len());

    let popdat = popdat::PopDat::import_all(&mut abstutil::Timer::new("importing popdat"));
    abstutil::write_binary("../data/shapes/popdat", &popdat).unwrap();
}
