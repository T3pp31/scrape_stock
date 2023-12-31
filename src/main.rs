use anyhow::Result;
use reqwest::blocking;
use scraper::{Html, Selector};
use std::env;
use std::fs::{self, create_dir_all, remove_file, File};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{self, Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;
use xcss::xcss::xcss;

const URL: &str = "https://kabutan.jp/stock/?code=";
const BOM: &[u8; 3] = &[0xEF, 0xBB, 0xBF]; // UTF-8

fn main() {
    //カレントディレクトリにあるinput.txtを絶対パスで取るための手法
    //exeで実行した際に，相対パスだとうまくいかないため，exeを実行した場所の絶対パスを取得し，input.txtを付け足した．
    let file_path = if let Ok(exe_path) = env::current_exe() {
        exe_path
            .parent()
            .map(|p| p.join("input.txt"))
            .unwrap_or_else(|| Path::new("input.txt").to_path_buf())
    } else {
        eprintln!("Error: Unable to get the current executable path!");
        return;
    };
    println!("File path: {:?}", file_path);

    //出力先のディレクトリを作成する
    let exe_path = env::current_exe().unwrap().parent().unwrap().join("output");
    let path = Path::new(exe_path.to_str().unwrap());
    remove_file(&exe_path);

    let writer_result = first_open(&path);
    let mut writer = match writer_result {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error opening file: {}", e);
            return;
        }
    };

    let stock_ids = read_stock_id(&file_path).expect("Failed to read input.txt");
    // let stock_ids = vec![
    //     "2914", "4502", "5020", "8306", "8593", "8766", "9432", "9433", "7164", "8566", "8058",
    //     "5021",
    // ];

    for stock_id in stock_ids {
        let document = search_stock(&stock_id).expect("Failed to fetch stock information");
        let stock_name = get_stock_info(&document, "//*[@id='stockinfo_i1']/div[1]/h2")
            .unwrap_or_else(|err| {
                eprintln!("Error getting stock name: {}", err);
                String::from("N/A")
            });

        let stock_price = get_stock_info(&document, "//*[@id='stockinfo_i1']/div[2]/span[2] ")
            .unwrap_or_else(|err| {
                eprintln!("Error getting stock price: {}", err);
                String::from("N/A")
            });
        let per = get_stock_info(&document, "//*[@id='stockinfo_i3']/table/tbody/tr[1]/td[1]")
            .unwrap_or_else(|err| {
                eprintln!("Error getting per: {}", err);
                String::from("N/A")
            });

        let pbr = get_stock_info(&document, "//*[@id='stockinfo_i3']/table/tbody/tr[1]/td[2]")
            .unwrap_or_else(|err| {
                eprintln!("Error getting pbr: {}", err);
                String::from("N/A")
            });
        let return_per =
            get_stock_info(&document, "//*[@id='stockinfo_i3']/table/tbody/tr[1]/td[3]")
                .unwrap_or_else(|err| {
                    eprintln!("Error getting return_per: {}", err);
                    String::from("N/A")
                });

        let predict_return = get_stock_info(
            &document,
            "//*[@id='kobetsu_right']/div[3]/table/tbody/tr[3]/th",
        )
        .unwrap_or_else(|err| {
            eprintln!("Error getting predict_return: {}", err);
            String::from("N/A")
        });

        let earning_per_share = get_stock_info(
            &document,
            "//*[@id='kobetsu_right']/div[3]/table/tbody/tr[3]/td[4]",
        )
        .unwrap_or_else(|err| {
            eprintln!("Error getting earning_per_share: {}", err);
            String::from("N/A")
        });

        let amount_return = get_stock_info(
            &document,
            "//*[@id='kobetsu_right']/div[3]/table/tbody/tr[3]/td[5]",
        )
        .unwrap_or_else(|err| {
            eprintln!("Error getting amount_return: {}", err);
            String::from("N/A")
        });

        println!("Stock Name: {}", stock_name);
        println!("Stock Price: {}", stock_price);
        println!("per:{}", per);
        println!("pbr:{}", pbr);
        println!("return_per:{}", return_per);
        println!("1株益{}", earning_per_share);
        println!("{}:{}", predict_return, amount_return);

        let data = format!(
            "{},{},{},{},{},{},{},{},{}",
            stock_id,
            stock_name,
            stock_price.replace(",", ""),
            per,
            pbr,
            return_per,
            earning_per_share,
            predict_return,
            amount_return
        )
        .to_string();

        write_to_csv(&data, &mut writer);

        //一定時間待機
        let duration = Duration::from_secs(5);
        sleep(duration);
    }
}

fn read_stock_id(path: &PathBuf) -> Result<Vec<String>, io::Error> {
    //保存用のリストを作成する
    let mut stock_ids: Vec<String> = Vec::new();

    // .txtファイルの中身を1行ずつ取り出す
    for result in BufReader::new(File::open(path)?).lines() {
        let line = result?;
        stock_ids.push(line);
    }
    Ok(stock_ids)
}

fn search_stock(stock_id: &str) -> Result<Html, reqwest::Error> {
    let get_url = format!("{}{}", URL, stock_id);
    let response = blocking::get(&get_url)?.text()?;
    Ok(Html::parse_document(&response))
}

fn get_stock_info(document: &Html, x_path: &str) -> Result<String, &'static str> {
    let css = xcss(x_path);
    let selector = Selector::parse(&css).map_err(|_| "Failed to parse selector")?;
    document
        .select(&selector)
        .next()
        .map(|data| data.text().collect::<Vec<_>>().join(""))
        .ok_or("Selector not found")
}

fn first_open(path: &Path) -> Result<BufWriter<File>, std::io::Error> {
    // ディレクトリが存在する場合は削除します。
    if path.exists() {
        fs::remove_dir_all(path);
    }

    // 新しいディレクトリを作成します。
    fs::create_dir_all(path);

    // ファイルを作成または既存のファイルを開きます。
    let file = File::create(path.join("output.csv"))?;
    println!("File opened successfully: {:?}", path);
    // BufWriterを使用してファイルへの書き込みを効率的に行います。

    let mut writer = BufWriter::new(file);
    writer.write_all(BOM);
    // ヘッダー行を書き込みます。
    write_to_csv(
        "株式番号,会社名,株価,PER,PBR,配当利回り,1株益(EPS),予想配当年,予想配当額",
        &mut writer,
    );

    Ok(writer)
}

fn write_to_csv(data: &str, writer: &mut BufWriter<File>) {
    writeln!(writer, "{}", data);
}
