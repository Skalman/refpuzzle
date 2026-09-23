import type { PointerKind } from "../lib/pointer.ts";

function plural(n: number, one: string, other: string): string {
  return n === 1 ? `${n} ${one}` : `${n} ${other}`;
}

/** The icon a How-to-play step is illustrated with. */
export type HelpIcon = "incorrect" | "correct" | "checkpoint";

/** Marking verb for the primary pointer: touch → "Tap", mouse → "Click". */
function tapVerb(p: PointerKind): string {
  return p === "coarse" ? "Tap" : "Click";
}

/** A natural question list: "#1", "#1 and #3", "#1, #2 and #3". */
export function qList(qis: number[]): string {
  const labels = qis.map((qi) => `#${qi + 1}`);
  if (labels.length <= 1) return labels[0] ?? "";
  return `${labels.slice(0, -1).join(", ")} and ${labels[labels.length - 1]}`;
}

export default {
  app: {
    title: "Refpuzzle",
    loading: "Loading...",
    noPuzzle: "No puzzle available for this date.",
  },
  puzzleList: {
    subtitle: "Self-referential logic puzzles",
    questions: "questions",
    solvedCount: (solved: number, total: number) => `${solved}/${total}`,
  },
  difficulty: {
    1: "Intro",
    2: "Easy",
    3: "Medium",
    4: "Hard",
    5: "Harder",
    6: "Expert",
  } as Record<number, string>,
  puzzle: {
    undo: "Undo",
    redo: "Redo",
    hint: "Hint",
    noNextStep: "No obvious next step. Try making an assumption.",
    checkpoint: "Checkpoint",
    // Checkpoint verdicts. The button doesn't warn that the press can be refused,
    // so the two refusals carry the gentle framing instead.
    checkpointSet: "Checkpoint set — everything so far holds up.",
    checkpointWrong: "Not yet — something's off further back.",
    // `n` counts marks, not steps: the pins and retractions in the range don't
    // establish anything.
    verifiedMarks: (n: number) => `Verified · ${n}`,
    verifiedTitle: (answered: number, questions: number) =>
      `Verified: ${answered} of ${questions} questions answered`,
    checkpointFailsTitle: (n: number) => plural(n, "refused checkpoint", "refused checkpoints"),
    solved: "Puzzle solved!",
    // The dialog's title when every question went clean.
    solvedPerfect: "🌈 Puzzle solved!",
    nextPuzzle: "Next puzzle",
    // Reopens the solved dialog from the banner, without the celebration.
    summary: "Summary",
    // The result card's caption under the time.
    solvedIn: "solved in",
    // The solved dialog's lines, one per measure: a label, and the numbers
    // behind it. The time bands, fastest first, and the history pair are
    // neutral — a slow solve reads as a style, not a rank; hints and oopsies
    // are where the teasing is allowed.
    solvedLines: {
      solvedIn: {
        hot: (time: string) => ({ label: "Hot!", detail: `solved in ${time}` }),
        speedy: (time: string) => ({ label: "Speedy", detail: `solved in ${time}` }),
        smooth: (time: string) => ({ label: "Smooth", detail: `solved in ${time}` }),
        deliberate: (time: string) => ({ label: "Deliberate", detail: `solved in ${time}` }),
      },
      pathfinder: { label: "Pathfinder", detail: "no hints needed" },
      peeker: (n: number) => ({ label: "Peeker", detail: plural(n, "hint", "hints") }),
      freeSpirit: { label: "Free spirit", detail: "no checkpoints" },
      doubleChecker: (n: number) => ({
        label: "Double-checker",
        detail: plural(n, "checkpoint", "checkpoints"),
      }),
      oopsie: (refused: number, pressed: number) => ({
        label: "Oopsie",
        detail: `${refused} of ${plural(pressed, "checkpoint", "checkpoints")} refused`,
      }),
      straightSolver: { label: "Straight solver", detail: "nothing undone" },
      timeTraveler: (n: number) => ({
        label: "Time traveler",
        detail: `reversed course ${n === 1 ? "once" : `${n} times`}`,
      }),
    },
    // Idle nudges (L2+): a callout on the button it names.
    nudge: {
      checkpoint: "Use the checkpoint to check your work!",
      hint: "Want a hint?",
    },
    start: "Start",
    solvedBadge: "Solved",
    playAgain: "Play again",
    // The armed label: a second press is what actually discards the solve.
    playAgainConfirm: "Clear board?",
    more: "More",
  },
  // The escalating key-diff notes, shared by the Hint button and a refused
  // checkpoint. `qi` is 0-based; `letter` is the answer the note names.
  mistake: {
    vague: "You made an error.",
    question: (qi: number) => `You made an error in #${qi + 1}.`,
    answer: (qi: number, letter: string) => `#${qi + 1} is not ${letter} — try a different answer.`,
    elim: (qi: number, letter: string) => `You incorrectly eliminated #${qi + 1} option ${letter}.`,
  },
  // L1 in-play coach: calm, self-fading lines shown in the board padding. `#Q`
  // and force-vs-eliminate wording are templated from the engine's next step.
  coach: {
    mentalModel:
      "Every question is about this grid's own answers — fill it so every statement comes out true.",
    markingGesture: (p: PointerKind) =>
      `${tapVerb(p)} once to eliminate, ${tapVerb(p).toLowerCase()} again to mark correct.`,
    // `qs` is a pre-formatted question list, e.g. "#1 and #3".
    lookForce: (qs: string) => `You can already pin down an answer — take a look at ${qs}.`,
    lookEliminate: (qs: string) => `You can already eliminate an option — take a look at ${qs}.`,
    lookGeneric: "Start with whichever question you can already work out.",
    guidedLead: "Here's one you can get:",
    mistakeAnswer: (q: number) => `Your answer to #${q} looks off.`,
    mistakeElim: (q: number) => `Your elimination on #${q} looks off.`,
  },
  // Hint panel: the navigation pointer the frontend owns — deduce-rule, question,
  // and option prose all come from Rust. `qis` are 0-based question indices.
  hint: {
    tryLooking: (qis: number[]) => `Try looking at ${qList(qis)}.`,
  },
  daily: {
    dayLabel: (num: number, date: string) => `Day #${num} — ${date}`,
    today: "Today",
    archive: "Archive",
    printAll: "Print all puzzles",
    // One archive day, spoken: the level rail carries the same state visually.
    // A stale day's rail reports only the levels needing a recheck, so its
    // label does too. The trailing clause names where the day opens, which the
    // tile shows nowhere else.
    archiveDay: (date: string, solved: number, levels: number, opens: string) =>
      `${date} — ${solved} of ${levels} solved — opens ${opens}`,
    archiveDayStale: (date: string, stale: number, opens: string) =>
      `${date} — ${plural(stale, "level needs", "levels need")} rechecking — opens ${opens}`,
  },
  backup: {
    button: "Sync and backup",
    downloadBackup: "Download backup",
    uploadBackup: "Upload backup",
    uploadPreview: "Upload preview",
    confirmUpload: "Confirm upload",
    cancel: "Cancel",
    ok: "OK",
    uploadFailed: (msg: string) => `Upload failed: ${msg}`,
    puzzlesInBackup: (n: number) => plural(n, "puzzle in file", "puzzles in file"),
    actions: {
      new: "New puzzle — will be added",
      "replace-completed": "Completed in backup — will update yours",
      "replace-longer": "More progress in backup — will update yours",
      "keep-completed": "Already completed — no change",
      "keep-longer": "You're further ahead — no change",
      identical: "Identical — no change",
    } as Record<string, string>,
  },
  aria: {
    close: "Close",
    dismiss: "Dismiss",
    more: "More",
    logo: "Refpuzzle logo",
  },
  header: {
    theme: "Theme",
    themeOptions: "Theme options",
    // Names where the press lands; the button's icon shows the mode it is in.
    themeToggle: {
      auto: "Use system theme",
      light: "Switch to light theme",
      dark: "Switch to dark theme",
    },
    themeModes: { auto: "Auto", light: "Light", dark: "Dark" },
  },
  share: {
    share: "Share",
    copyLink: "Copy link",
    copied: "Copied!",
    // What the link opens: the app, this puzzle blank, or this puzzle with the board so far.
    opens: "What the link opens",
    modes: { app: "App", puzzle: "Puzzle", progress: "Puzzle progress" },
    copyText: "Copy text",
    // The pasteable result: headline, one square per question, then the link.
    // `time` is null for a solve that was never timed on this device.
    resultHeadline: (day: number, level: string, time: string | null) =>
      [`Refpuzzle Day #${day}`, level, ...(time ? [time] : [])].join(" · "),
    outcomeEmoji: { clean: "🟩", hinted: "🟨", caught: "🟥" },
    // Leads a perfect row, on the card and in the pasted text.
    perfectEmoji: "🌈",
    // Every square clean: the row is drawn from these instead, red to purple.
    rainbowEmoji: ["🟥", "🟧", "🟨", "🟩", "🟦", "🟪"],
  },
  install: {
    button: "Install app",
    shareApp: "Share app",
    iosSafari: 'Tap the Share button, then "Add to Home Screen"',
    androidFirefox: 'Tap the menu button (⋮), then "Add app to Home screen"',
    qrPrompt: "Scan to open on another device",
  },
  help: {
    title: "How to play",
    goal: "Every question is about this grid's own answers — fill it so every statement comes out true.",
    // `icon` names the illustration the step carries.
    howToPlaySteps: (p: PointerKind): { text: string; icon?: HelpIcon }[] => [
      { text: `${tapVerb(p)} an option to eliminate it`, icon: "incorrect" },
      { text: `${tapVerb(p)} again to mark it your answer`, icon: "correct" },
      { text: `${tapVerb(p)} once more to clear it` },
      { text: "The bar beside each question shows the answer's validity" },
      { text: "Set a checkpoint to check and lock in your marks so far", icon: "checkpoint" },
    ],
    whatIs: "What is a self-referential puzzle?",
    descriptionParagraphs: [
      'Each question refers to the puzzle itself: how many questions have a certain answer, which answers appear where, and so on. There is exactly one combination of answers that satisfies every question; this fact may help you find the answer to a particular question. Note that "answers" means the answers you give — the puzzle is entirely self-contained.',
      "Solving one requires logic and deduction: you often can't answer a question in isolation, so you work through the puzzle iteratively — eliminating wrong options, making tentative selections, and revising as new constraints emerge.",
      "The format was created by mathematician Jim Propp, whose original Self-Referential Aptitude Test had 20 questions and only one that could be solved without reference to the others.",
    ],
  },
  sync: {
    title: "Sync devices",
    description: "Transfer your progress between two devices.",
    start: "Start sync",
    enterCode: "Have a code?",
    join: "Join",
    waiting: "Waiting for other device...",
    codePlaceholder: "123456",
    scanQr: "Scan QR code",
    expired: "Code expired or not found",
    tooBusy: "Too busy, try again later",
    error: "Sync failed",
  },
  privacy: {
    link: "Privacy",
    title: "Privacy",
    paragraphs: [
      "Your progress and settings are stored in your browser and stay on your device.",
      "When you solve a puzzle, anonymous stats (puzzle, time, hints used, browser type) are sent to this site. There are no cookies, no user identifiers, and no third-party trackers.",
      "The web server keeps standard access logs, including IP addresses, for about 30 days.",
    ],
    // Followed by the contact address, derived from the site's hostname.
    contactPrompt: "Questions:",
  },
  contact: {
    link: "Made with ♥ by Dan",
    title: "Hello!",
    // Followed by the contact address.
    body: "Found a bug or have other feedback? Feel free to say hello!",
  },
  notFound: {
    noPuzzle: "No puzzle",
    backToToday: "Back to today",
    title: "404",
    pageNotFound: "Page not found",
    backToPuzzles: "Back to puzzles",
  },
  keyboard: {
    title: "Keyboard shortcuts",
    navigation: "Navigation",
    actions: "Actions",
    general: "General",
    jumpToQuestion: "Jump to question",
    prevQuestion: "Previous question",
    nextQuestion: "Next question",
    moveOptions: "Move between options",
    selectOption: "Select option A–E",
    toggleOption: "Toggle option",
    prevNextDifficulty: "Previous/next difficulty",
    undo: "Undo",
    redo: "Redo",
    hint: "Hint",
    checkpoint: "Checkpoint",
    closeCancel: "Close/cancel",
    toggleHelp: "Toggle this help",
    navigateSections: "Navigate between sections",
  },
} as const;
