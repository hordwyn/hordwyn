Phase 1 server API (built by WP1-server, uncommitted in the tree; DEC-204..212; read web/src/api.rs, contracts/src/browse.rs for exact types):
Server fns (web::api): kinds() -> KindsDto{groups[{id,label_key,kinds}], kinds[{id,group,label_key,progress_model,statuses,facets}]};
library_counts() -> LibraryCountsDto{total, groups, kinds, statuses: [CountDto{key,count}], cells[{kind,group,status,count}]};
library_query(LibraryQueryDto{group,kinds,statuses,tags,facets[{attribute,value}],q,sort(last_activity|title|added|year|rating|plays),descending,offset,limit}) -> LibraryRowsDto{rows[LibraryRowDto{entry,item,group,last_activity,tags,summary,next_step}],total,offset,limit};
set_status(item_id,status); tags() -> [TagDto{tag,entries}]; add_tag/remove_tag(item_id,tag) -> entry tags; rename_tag(from,to);
set_entry_note(item_id, Option<String>) (stored in entry.notes); undo_progress(event_id) -> compensating event (24 h window, own events);
activity_feed(ActivityQueryDto{before,limit,group,kind,person,row_type,shared,year}) -> ActivityPageDto{rows[ActivityRowDto{at,row_type(progress|session),item_id,title,kind,group,progress,event_id,session_id,participants[{companion_id,name,is_me}],shared,owner_name,undoable}],next};
stats_totals() -> StatsTotalsDto{overall, groups: [TotalsDto{group,entries,planned,in_progress,completed,paused,dropped,amounts{events,completions,finished,episodes,pages,plays,minutes}}]};
stats_series(group: Option<String> ("all"|group id), bucket: Option<String> (month|year)) -> StatsSeriesDto{group,bucket,points[{bucket,amounts}]};
home() -> HomeDto{library_total, continue_rows(6), up_next[{row,reason: next_in_series|planned}](6), activity(5 rows,next), stats};
item_page(id) -> ItemPageDto now with tags, summary (ProgressSummaryDto incl. fraction 0..1), next_step (NextStepDto{action: finish|episode|caught_up|page|position|session, label_key next-step-*, again, season, episode, page, chapter, pages, chapters, seconds, total_seconds, plays}); progress = effective history (undone left out).
Fluent keys exist: kind-group-<id>, kind-group-all, next-step-* (next-step-episode takes $season $episode).
Not built (do not invent): sorts Progress/Next release/Priority; feed rows for link requests/imports/added; per-person series; airing status; tags in search index; admin nudge (phase 2).

Reusable web widgets from WP1-library (uncommitted, in the tree):
web::next_step: NextStepButton(item_id, title, step: NextStepDto, compact, on_done, class) (label, action, toast with Undo, inline error); next_step_label(&I18n,&NextStepDto); step_progress; progress_text(&I18n,&ProgressSummaryDto,&NextStepDto); percent(f32); run_step(StepCtx::current(), LoggedStep{item_id,title,label,progress}, busy, error, on_done) for log sheets.
web::widgets: PosterCard(row: LibraryRowDto, statuses, on_change, on_tags); QuickMenu; TagEditor(target: RwSignal<Option<LibraryRowDto>>, known, on_change) dialog; TagBadges(tags,max); Cover{square}.
web::library_state: LibraryState (URL state), square_art, SORTS, PAGE_SIZE.
ui: ToastAction::new(label, Callback), Toasts::push_action(title, desc, action), ACTION_TOAST_SECS (10 s); DropdownMenu id + trigger_class.
Fluent keys to share: library-logged, library-undo, library-undone, library-next-step-named, library-progress-*, library-mark, library-status-changed, status-endless.
Lesson: StoredValue::new_local / Action::new_local abort SSR streams in the real server (thread-local dropped on another thread) - use the plain versions. log_progress server fn takes JSON input.

Reusable from WP1-home (uncommitted): web::history::ActivityRow(row, dated, on_change) (sentence row with Undo and Copy to my log); web::history_state (Day, group_by_day, day_heading, HistoryState); web::charts (BarList, ColumnChart, nice_ticks, ...); web::home::TotalsCharts; web::next_step::undo_event(&StepCtx, event_id, on_done); I18n::day_month.
Gotchas: read resources only inside <Suspense>/<Transition> (outside breaks hydration and kills click handlers); no DOM/test ids starting with a Fluent key prefix (xtask treats them as dynamic key families).
