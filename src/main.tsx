import { render } from "preact";
import "./index.css"; // oxlint-disable-line import/no-unassigned-import
import { App } from "./App.tsx";

render(<App />, document.getElementById("app")!);
