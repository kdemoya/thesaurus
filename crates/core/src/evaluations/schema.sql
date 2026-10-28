CREATE TABLE IF NOT EXISTS eval_suites(id TEXT PRIMARY KEY, version INTEGER NOT NULL, project_id TEXT);
CREATE TABLE IF NOT EXISTS eval_suite_versions(suite_id TEXT NOT NULL, version INTEGER NOT NULL, record TEXT NOT NULL, PRIMARY KEY(suite_id,version));
CREATE TABLE IF NOT EXISTS eval_runs(id TEXT PRIMARY KEY, suite_id TEXT NOT NULL, project_id TEXT NOT NULL, record TEXT NOT NULL, created_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS eval_run_project ON eval_runs(project_id,created_at);
CREATE TABLE IF NOT EXISTS eval_reviews(id TEXT PRIMARY KEY, run_id TEXT NOT NULL, record TEXT NOT NULL, created_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS eval_review_run ON eval_reviews(run_id,created_at);

CREATE TABLE IF NOT EXISTS eval_judge_receipts(runtime_id TEXT PRIMARY KEY,eval_run_id TEXT NOT NULL,case_id TEXT NOT NULL,prompt_hash TEXT NOT NULL,estimated_input_tokens INTEGER NOT NULL,created_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS eval_judge_parent ON eval_judge_receipts(eval_run_id);

CREATE TABLE IF NOT EXISTS eval_replay_plans(id TEXT PRIMARY KEY,parent_run_id TEXT NOT NULL,record TEXT NOT NULL,created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS eval_replay_receipts(runtime_id TEXT PRIMARY KEY,plan_id TEXT NOT NULL,case_id TEXT NOT NULL,prompt_hash TEXT NOT NULL,estimated_input_tokens INTEGER NOT NULL,created_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS eval_replay_parent ON eval_replay_receipts(plan_id);
CREATE TABLE IF NOT EXISTS eval_replay_finalizations(runtime_id TEXT PRIMARY KEY,eval_run_id TEXT NOT NULL);
