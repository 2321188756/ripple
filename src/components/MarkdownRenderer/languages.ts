import { PrismLight as SyntaxHighlighter } from "react-syntax-highlighter";
import js from "react-syntax-highlighter/dist/esm/languages/prism/javascript";
import ts from "react-syntax-highlighter/dist/esm/languages/prism/typescript";
import jsx from "react-syntax-highlighter/dist/esm/languages/prism/jsx";
import tsx from "react-syntax-highlighter/dist/esm/languages/prism/tsx";
import python from "react-syntax-highlighter/dist/esm/languages/prism/python";
import rust from "react-syntax-highlighter/dist/esm/languages/prism/rust";
import bash from "react-syntax-highlighter/dist/esm/languages/prism/bash";
import json from "react-syntax-highlighter/dist/esm/languages/prism/json";
import css from "react-syntax-highlighter/dist/esm/languages/prism/css";
import markdown from "react-syntax-highlighter/dist/esm/languages/prism/markdown";
import yaml from "react-syntax-highlighter/dist/esm/languages/prism/yaml";

export function registerLanguages() {
  SyntaxHighlighter.registerLanguage("javascript", js);
  SyntaxHighlighter.registerLanguage("js", js);
  SyntaxHighlighter.registerLanguage("typescript", ts);
  SyntaxHighlighter.registerLanguage("ts", ts);
  SyntaxHighlighter.registerLanguage("jsx", jsx);
  SyntaxHighlighter.registerLanguage("tsx", tsx);
  SyntaxHighlighter.registerLanguage("python", python);
  SyntaxHighlighter.registerLanguage("py", python);
  SyntaxHighlighter.registerLanguage("rust", rust);
  SyntaxHighlighter.registerLanguage("rs", rust);
  SyntaxHighlighter.registerLanguage("bash", bash);
  SyntaxHighlighter.registerLanguage("sh", bash);
  SyntaxHighlighter.registerLanguage("shell", bash);
  SyntaxHighlighter.registerLanguage("json", json);
  SyntaxHighlighter.registerLanguage("css", css);
  SyntaxHighlighter.registerLanguage("markdown", markdown);
  SyntaxHighlighter.registerLanguage("md", markdown);
  SyntaxHighlighter.registerLanguage("yaml", yaml);
  SyntaxHighlighter.registerLanguage("yml", yaml);
}
