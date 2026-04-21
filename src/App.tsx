import { LocationProvider, Router, Route } from "preact-iso";
import { PuzzleList } from "./components/PuzzleList.tsx";
import { PuzzleView } from "./components/PuzzleView.tsx";
import { About } from "./components/About.tsx";

function NotFound() {
  return (
    <div class="not-found">
      <h1>404</h1>
      <p>Page not found</p>
      <a href="/">Back to puzzles</a>
    </div>
  );
}

export function App() {
  return (
    <LocationProvider>
      <div class="page">
        <Router>
          <Route path="/" component={PuzzleList} />
          <Route path="/puzzle/:id" component={PuzzleView} />
          <Route path="/about" component={About} />
          <Route default component={NotFound} />
        </Router>
      </div>
    </LocationProvider>
  );
}
