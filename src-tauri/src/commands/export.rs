use crate::commands::repos::{ApiResult, DbState};
use crate::db::coverage as db_cov;
use tauri::State;

#[tauri::command]
pub fn export_csv(
    state: State<DbState>,
    repo_id: Option<i64>,
    include_files: Option<bool>,
) -> ApiResult<String> {
    let conn = state.0.lock().unwrap();
    let rows = match db_cov::all_runs_for_export(&conn, repo_id) {
        Ok(r) => r,
        Err(e) => return ApiResult::err(e),
    };

    let mut csv = String::from("repo,org,date,overall_coverage_pct,lines_covered,lines_total\n");
    for (name, org, date, pct, covered, total) in &rows {
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            escape_csv(name),
            escape_csv(org),
            escape_csv(date),
            pct.map_or(String::new(), |p| format!("{:.2}", p)),
            covered.map_or(String::new(), |c| c.to_string()),
            total.map_or(String::new(), |t| t.to_string()),
        ));
    }

    if include_files.unwrap_or(false) {
        // Append per-file breakdown for each run
        let run_ids: Vec<i64> = {
            let mut stmt = conn
                .prepare("SELECT id FROM coverage_runs WHERE status='success' ORDER BY started_at DESC")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .flatten()
                .collect()
        };

        csv.push('\n');
        csv.push_str("run_id,file_path,coverage_pct,lines_covered,lines_total\n");
        for run_id in run_ids {
            if let Ok(files) = db_cov::get_file_coverage(&conn, run_id) {
                for f in files {
                    csv.push_str(&format!(
                        "{},{},{},{},{}\n",
                        run_id,
                        escape_csv(&f.file_path),
                        f.coverage_percent.map_or(String::new(), |p| format!("{:.2}", p)),
                        f.lines_covered.map_or(String::new(), |c| c.to_string()),
                        f.lines_total.map_or(String::new(), |t| t.to_string()),
                    ));
                }
            }
        }
    }

    ApiResult::ok(csv)
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
