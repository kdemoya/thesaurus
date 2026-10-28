CREATE TABLE priority_preferences (project_id TEXT NOT NULL REFERENCES project_briefs(id),task_id TEXT NOT NULL REFERENCES tasks(id),record TEXT NOT NULL,PRIMARY KEY(project_id,task_id));
CREATE TABLE priority_reviews (sequence INTEGER PRIMARY KEY AUTOINCREMENT,id TEXT NOT NULL UNIQUE,project_id TEXT NOT NULL REFERENCES project_briefs(id),record TEXT NOT NULL);
CREATE INDEX priority_reviews_project ON priority_reviews(project_id,sequence);
CREATE TABLE waiting_for (id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES project_briefs(id),record TEXT NOT NULL);
CREATE INDEX waiting_for_project ON waiting_for(project_id);
CREATE TABLE waiting_history (sequence INTEGER PRIMARY KEY AUTOINCREMENT,waiting_id TEXT NOT NULL REFERENCES waiting_for(id),record TEXT NOT NULL);
CREATE INDEX waiting_history_owner ON waiting_history(waiting_id,sequence);
PRAGMA user_version=5;
