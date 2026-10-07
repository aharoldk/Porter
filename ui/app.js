const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const appWindow = window.__TAURI__.window.getCurrentWindow();

const urlInput = document.querySelector("#url");
const browserSelect = document.querySelector("#browser");
const profileSelect = document.querySelector("#profile");
const presetList = document.querySelector("#presets");
const labelInput = document.querySelector("#label");
const error = document.querySelector("#error");

let browsers = [];
let presets = [];

function showError(message) {
  error.hidden = !message;
  error.textContent = message || "";
}

function selectedBrowser() {
  return browsers.find((browser) => browser.id === browserSelect.value);
}

function fillBrowsers() {
  const current = browserSelect.value;
  browserSelect.replaceChildren();
  if (browsers.length === 0) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = "None yet";
    browserSelect.append(option);
    return;
  }
  for (const browser of browsers) {
    const option = document.createElement("option");
    option.value = browser.id;
    option.textContent = browser.name;
    browserSelect.append(option);
  }
  if (browsers.some((browser) => browser.id === current)) {
    browserSelect.value = current;
  }
}

async function fillProfiles() {
  const browser = selectedBrowser();
  const current = profileSelect.value;
  profileSelect.replaceChildren();
  profileSelect.disabled = !browser || browser.kind === "safari";
  if (!browser || browser.kind === "safari") {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = browser ? "—" : "—";
    profileSelect.append(option);
    return;
  }
  const profiles = await invoke("list_profiles", { browserId: browser.id });
  if (profiles.length === 0) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = "No profiles";
    profileSelect.append(option);
    return;
  }
  for (const profile of profiles) {
    const option = document.createElement("option");
    option.value = profile.id;
    option.textContent = profile.name;
    profileSelect.append(option);
  }
  if (profiles.some((p) => p.id === current)) {
    profileSelect.value = current;
  }
}

function fillPresets() {
  presetList.replaceChildren();
  if (presets.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty";
    empty.textContent = "None yet";
    presetList.append(empty);
    return;
  }
  for (const preset of presets) {
    const item = document.createElement("li");
    const open = document.createElement("button");
    open.type = "button";
    open.className = "name";
    open.textContent = preset.label;
    open.addEventListener("click", () => run(() => invoke("launch_preset", { id: preset.id })));
    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "x";
    remove.textContent = "×";
    remove.title = "Delete";
    remove.addEventListener("click", () =>
      run(async () => {
        await invoke("delete_preset", { id: preset.id });
        presets = presets.filter((item) => item.id !== preset.id);
        fillPresets();
      }),
    );
    item.append(open, remove);
    presetList.append(item);
  }
}

async function refresh() {
  const state = await invoke("get_state");
  browsers = state.browsers;
  presets = state.presets;
  fillBrowsers();
  await fillProfiles();
  fillPresets();
}

async function run(action) {
  showError("");
  try {
    await action();
  } catch (err) {
    showError(typeof err === "string" ? err : "Something went wrong");
  }
}

document.querySelector("#close").addEventListener("click", () => appWindow.hide());

document.querySelector("#drag").addEventListener("mousedown", (event) => {
  if (event.target.closest("button")) return;
  appWindow.startDragging();
});

browserSelect.addEventListener("change", () => run(fillProfiles));

document.querySelector("#add").addEventListener("click", () =>
  run(async () => {
    const browser = await invoke("add_browser");
    if (!browser) return;
    await refresh();
    browserSelect.value = browser.id;
    await fillProfiles();
  }),
);

document.querySelector("#remove").addEventListener("click", () =>
  run(async () => {
    const browser = selectedBrowser();
    if (!browser) return;
    await invoke("remove_browser", { id: browser.id });
    await refresh();
  }),
);

document.querySelector("#launch").addEventListener("click", () =>
  run(async () => {
    const browser = selectedBrowser();
    if (!browser) throw "Add a browser first";
    await invoke("launch_url", {
      browserId: browser.id,
      profile: profileSelect.value || null,
      url: urlInput.value,
    });
    await appWindow.hide();
  }),
);

document.querySelector("#save").addEventListener("click", () =>
  run(async () => {
    const browser = selectedBrowser();
    if (!browser) throw "Add a browser first";
    const preset = await invoke("save_preset", {
      label: labelInput.value,
      url: urlInput.value,
      browserId: browser.id,
      profile: profileSelect.value || null,
    });
    presets.push(preset);
    labelInput.value = "";
    fillPresets();
  }),
);

urlInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") document.querySelector("#launch").click();
});

async function fillIncomingUrl(url) {
  urlInput.value = url;
  urlInput.focus();
  urlInput.select();
  await fillProfiles();
  invoke("auto_launch_url", {
    url,
    browserId: browserSelect.value || null,
    profile: profileSelect.value || null,
  }).catch((err) => showError(String(err)));
}

listen("open-url", (event) => fillIncomingUrl(event.payload));

async function takePending() {
  const pending = await invoke("take_pending_url");
  if (pending) fillIncomingUrl(pending);
}

refresh().catch((err) => showError(String(err)));
takePending().catch(() => {});
