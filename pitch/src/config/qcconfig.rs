use clap::{Parser};

/// Quality checker configuration
#[derive(Parser)]
#[command(author, version)]
pub struct QCConfiguration {
    #[arg(long)]
    /// the path to the .wav signal to check
    ps: String,

    #[arg(long)]
    /// the path to the .wav oracle signal
    po: String,

    #[arg(long)]
    /// the threshold of the euclidian distance
    th: f32,
}

impl QCConfiguration {

    /// Returns a new QCConfiguration from the given parameters
    fn new(path_signal: &str, path_oracle: &str, threshold: f32) -> Self {
        Self {
            ps: path_signal.to_string(),
            po: path_oracle.to_string(),
            th: threshold
        }
    }

    /// Returns a slice of the path to the signal
    pub fn path_to_signal(&self) -> &str {
        &self.ps[..]
    }

    /// Returns a slice of the path to the oracle
    pub fn path_to_oracle(&self) -> &str {
        &self.po[..]
    }

    /// Returns the threshold value
    pub fn threshold(&self) -> f32 {
        self.th
    }
}

/// Parses the arguments from the `main()` function and loads the corresponding
/// configuration
pub fn parse_config(args: &[String]) -> Result<QCConfiguration, &'static str> {
    if args.len() != 3 {
        return Err("Illegal number of parameters");
    }
    let ps = &(args[0])[..];
    let po = &(args[1])[..];
    let th = match args[2].parse::<f32>() {
        Ok(n) => n,
        _ => return Err("Illegal threshold value"),
    };
    if th < 0.0 {
        return Err("Illegal threshold value");
    }
    Ok(QCConfiguration::new(ps, po, th))
}
