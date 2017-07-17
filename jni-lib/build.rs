#![feature(catch_expr)]

extern crate kohi;

use kohi::Kohi;

fn main() {
    let res: kohi::Result<()> = do catch {
        let kohi = Kohi::new()?;
        kohi.compile()?.package("../aurora-native.jar",None)?;
        Ok(())
    };
    res.expect("Build failed");
}