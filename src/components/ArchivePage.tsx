import { AppHeader } from "./AppHeader.tsx";
import { useBackupFlow, BackupDialogs } from "./BackupFlow.tsx";
import { classNames } from "../lib/classNames.ts";
import { useRevalidated } from "../lib/hooks.ts";
import { useToday } from "../lib/today.ts";
import { LEVELS, dateStrFromOffset, isValidDate } from "../puzzles/daily.ts";
import { dayStates, isSolved, resumeLevel } from "../puzzles/progress.ts";
import { t } from "../i18n/index.ts";

interface WeekInfoLocale extends Intl.Locale {
  getWeekInfo?: () => { firstDay: number };
  weekInfo?: { firstDay: number };
}

/**
 * The locale's first day of the week, 1 (Mon) … 7 (Sun). Some engines expose
 * the week info as a getter rather than a method and older ones as neither, so
 * probe both and fall back to Monday (ISO).
 */
function firstWeekday(): number {
  try {
    const locale: WeekInfoLocale = new Intl.Locale(navigator.language);
    return locale.getWeekInfo?.().firstDay ?? locale.weekInfo?.firstDay ?? 1;
  } catch {
    return 1;
  }
}

/** The seven column headings, starting at the locale's first weekday. */
function weekdayNames(first: number): string[] {
  // 2024-01-07 was a Sunday, so adding an ISO weekday number (Sun = 7 → +0)
  // lands on that weekday.
  return Array.from({ length: 7 }, (_, column) => {
    const iso = ((first - 1 + column) % 7) + 1;
    return new Date(2024, 0, 7 + (iso % 7)).toLocaleDateString(undefined, { weekday: "short" });
  });
}

// The locale can't change while the page is open, so resolve both once.
const WEEK_START = firstWeekday();
const WEEKDAYS = weekdayNames(WEEK_START);

function formatMonth(year: number, month: number): string {
  return new Date(year, month - 1).toLocaleString(undefined, { month: "long", year: "numeric" });
}

function formatDay(dateStr: string): string {
  return new Date(dateStr + "T00:00:00").toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

/** One day of the archive: its date, a six-level track, and a done-ness tint. */
function ArchiveDay({ dateStr, day, isToday }: { dateStr: string; day: number; isToday: boolean }) {
  const s = t();
  const states = dayStates(dateStr);
  const solved = states.filter(isSolved).length;
  const stale = states.filter((state) => state.stale).length;
  const started = states.some((state) => state.started);
  const target = resumeLevel(states);

  // Stale wins the tint: it needs the alarm, and its track stops reporting the
  // other levels, so the label stops naming them too.
  const tint = stale > 0 ? "stale" : solved === LEVELS.length ? "done" : started ? "partial" : "";
  const dateLabel = isToday ? s.daily.today : formatDay(dateStr);
  const label =
    stale > 0
      ? s.daily.archiveDayStale(dateLabel, stale, s.difficulty[target])
      : s.daily.archiveDay(dateLabel, solved, LEVELS.length, s.difficulty[target]);

  return (
    <a
      href={`/${dateStr}/${target}`}
      class={classNames("archive-day", tint, isToday && "today")}
      aria-label={label}
    >
      <span class="archive-daynum">{day}</span>
      <span class="archive-track" aria-hidden="true">
        {LEVELS.map((level, i) => {
          const state = states[i];
          const levelTint = state.stale
            ? "stale"
            : state.completed
              ? "solved"
              : state.started
                ? "started"
                : "";
          return <span key={level} class={classNames("archive-level", levelTint)} />;
        })}
      </span>
    </a>
  );
}

/**
 * One month as a weekday-column grid, latest week first so the whole page runs
 * backwards in time. A day the archive doesn't reach — before it started, or
 * still ahead of today — leaves its slot blank, and a week holding no day at
 * all drops out.
 */
function ArchiveMonth({ ym, today }: { ym: string; today: string }) {
  const year = Number(ym.slice(0, 4));
  const month = Number(ym.slice(5, 7));
  const dayCount = new Date(year, month, 0).getDate();
  // getDay() is Sun = 0, the week-info numbering is Sun = 7.
  const lead = (new Date(year, month - 1, 1).getDay() - (WEEK_START % 7) + 7) % 7;

  const slots: (string | null)[] = Array.from({ length: lead }, () => null);
  for (let day = 1; day <= dayCount; day++) {
    const dateStr = `${ym}-${String(day).padStart(2, "0")}`;
    slots.push(isValidDate(dateStr) ? dateStr : null);
  }
  // Both ends padded to whole weeks: a short row placed first would slide its
  // days out of their weekday columns.
  while (slots.length % 7 !== 0) slots.push(null);

  const weeks: (string | null)[][] = [];
  for (let i = 0; i < slots.length; i += 7) {
    const week = slots.slice(i, i + 7);
    if (week.some(Boolean)) weeks.push(week);
  }
  weeks.reverse();

  return (
    <section class="archive-month">
      <h3 class="archive-month-title">{formatMonth(year, month)}</h3>
      <div class="archive-weekdays" aria-hidden="true">
        {WEEKDAYS.map((name) => (
          <span key={name}>{name}</span>
        ))}
      </div>
      <div class="archive-grid">
        {weeks.flat().map((dateStr, i) =>
          dateStr === null ? (
            // oxlint-disable-next-line react/no-array-index-key
            <span key={`blank${i}`} />
          ) : (
            <ArchiveDay
              key={dateStr}
              dateStr={dateStr}
              day={Number(dateStr.slice(8))}
              isToday={dateStr === today}
            />
          ),
        )}
      </div>
    </section>
  );
}

export function ArchivePage() {
  const s = t();
  const backup = useBackupFlow();
  const today = useToday();
  useRevalidated();

  // Newest month first, so today sits at the top of the scroll.
  const months: string[] = [];
  for (let i = 0; ; i++) {
    const dateStr = dateStrFromOffset(i);
    if (!isValidDate(dateStr)) break;
    const ym = dateStr.slice(0, 7);
    if (months[months.length - 1] !== ym) months.push(ym);
  }

  return (
    <>
      <AppHeader onBackup={backup.openBackup} />

      <div class="archive-page">
        <h2>{s.daily.archive}</h2>
        {months.map((ym) => (
          <ArchiveMonth key={ym} ym={ym} today={today} />
        ))}
      </div>

      <BackupDialogs backup={backup} exportFilename="refpuzzle-backup.json" />
    </>
  );
}
