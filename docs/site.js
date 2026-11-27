"use strict";

const calm = matchMedia("(prefers-reduced-motion: reduce)").matches;

function selectTab(tab, focus) {
  const list = tab.closest('[role="tablist"]');
  for (const other of list.querySelectorAll('[role="tab"]')) {
    const on = other === tab;
    other.setAttribute("aria-selected", String(on));
    other.tabIndex = on ? 0 : -1;
    document.getElementById(other.getAttribute("aria-controls")).hidden = !on;
  }
  if (focus) tab.focus();
}

for (const list of document.querySelectorAll('[role="tablist"]')) {
  const tabs = [...list.querySelectorAll('[role="tab"]')];
  list.addEventListener("click", (event) => {
    const tab = event.target.closest('[role="tab"]');
    if (!tab) return;
    selectTab(tab);
    type(tab);
  });
  list.addEventListener("keydown", (event) => {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
    const at = tabs.indexOf(event.target);
    if (at < 0) return;
    let next = null;
    if (step) next = tabs[(at + step + tabs.length) % tabs.length];
    if (event.key === "Home") next = tabs[0];
    if (event.key === "End") next = tabs[tabs.length - 1];
    if (!next) return;
    event.preventDefault();
    selectTab(next, true);
  });
}

// Types the chosen question. The complete text is always in the document.
let typing = 0;
function type(tab) {
  const target = document.querySelector(`#${tab.getAttribute("aria-controls")} .ask q`);
  if (!target || calm) return;
  const text = tab.textContent;
  clearInterval(typing);
  for (const button of tab.parentElement.querySelectorAll('[role="tab"]')) {
    document.querySelector(`#${button.getAttribute("aria-controls")} .ask q`).textContent = button.textContent;
  }
  let length = 0;
  target.textContent = "";
  typing = setInterval(() => {
    length += 1;
    target.textContent = text.slice(0, length);
    if (length >= text.length) clearInterval(typing);
  }, 14);
}

const platform = document.querySelector("[data-platform]");
if (platform) {
  const agent = navigator.userAgent;
  if (/Windows/.test(agent)) selectTab(document.getElementById("t-ps"));
  else if (/Linux|X11/.test(agent)) selectTab(document.getElementById("t-sh"));
}

const slider = document.getElementById("budget");
if (slider) {
  const packs = [...document.querySelectorAll("[data-budget]")];
  const label = document.querySelector("#budget-value");
  const command = document.querySelector("#budget-command");
  const show = () => {
    const index = Number(slider.value);
    const budget = packs[index].dataset.budget;
    packs.forEach((pack, position) => {
      pack.hidden = position !== index;
    });
    label.textContent = `${Number(budget).toLocaleString("en-US")} tokens`;
    command.textContent = budget;
    slider.setAttribute("aria-valuetext", label.textContent);
  };
  slider.addEventListener("input", show);
  show();
}

for (const button of document.querySelectorAll(".copy")) {
  button.addEventListener("click", async () => {
    const text = button.parentElement.querySelector("pre").textContent;
    let done = "Copied";
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      done = "Press Ctrl+C";
      getSelection().selectAllChildren(button.parentElement.querySelector("pre"));
    }
    button.textContent = done;
    document.getElementById("announce").textContent = done;
    setTimeout(() => {
      button.textContent = "Copy";
    }, 1800);
  });
}
