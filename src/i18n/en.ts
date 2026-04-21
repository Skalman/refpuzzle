export default {
  app: {
    title: "Refpuzzle",
  },
  puzzleList: {
    subtitle: "Self-referential logic puzzles",
    questions: "questions",
    solved: "Solved",
  },
  difficulty: {
    1: "Beginner",
    2: "Easy",
    3: "Medium",
    4: "Hard",
    5: "Expert",
  },
  puzzle: {
    undo: "Undo",
    redo: "Redo",
    reset: "Reset",
    share: "Share",
    hint: "Hint",
    solved: "Puzzle solved!",
    back: "All puzzles",
  },
  about: {
    title: "About",
    howToPlay: "How to Play",
    howToPlaySteps: [
      "Click an answer once to mark it incorrect",
      "Click it twice to mark it correct (there is only one correct answer for each question)",
      "The adjacent bar indicates the question's logical validity: green for correct, red for incorrect",
    ],
    howToSolve: "How to Solve",
    howToSolveSteps: [
      "Start by reading through all the questions to get a sense of the quiz structure",
      "Look for alternatives that are obviously incorrect and mark them as such",
      "Solve questions that seem straightforward or that give clues about multiple other questions",
      "Keep track of potential answers for each question as you go along",
      "Revisit previous questions as new information becomes available",
      "The game ends when all the questions are logically correct (green bar)",
    ],
    whatIs: "What is a self-referential quiz?",
    description:
      "A self-referential puzzle is a type of puzzle where the questions refer to the puzzle itself or other questions within the same puzzle. The answers often depend on the content or structure of the puzzle, making it a meta-puzzle. Solving the puzzle usually involves logic and deductive reasoning, as you have to consider the implications of each answer on the rest of the puzzle.",
  },
} as const;
