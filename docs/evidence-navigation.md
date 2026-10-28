# Exact evidence navigation (F01)

Action evidence buttons and the Work table's Enter shortcut open the immutable cited document revision and its exact one-based line. Shared citation buttons in briefs, project focus, meetings, annotations and other evidence views use the same path.

The viewer highlights the cited line, scrolls it into the center and moves keyboard focus to it after rendering. Its footer identifies the original document and immutable revision. The citation anchor also initializes the manual-note position, so a note can be added at the passage being inspected. Clicking another line changes the note anchor without rewriting the original citation.

Direct source opens and generic search-result opens display the requested document revision without asserting a particular cited line. Invalid or unavailable citation lines display an error. A request sequence prevents an older source response from replacing a newer source selection. Scrolling checks the rendered document and revision before moving focus.

The same shared UI runs in the browser and desktop webview. This implementation uses document line evidence; it does not assert audio timestamps for transcripts that do not contain timestamp metadata.
