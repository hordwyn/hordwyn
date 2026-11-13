// Theme button of the website: toggles the `dark` class and remembers the choice.
(function () {
  var root = document.documentElement;
  var button = document.querySelector("[data-theme-toggle]");
  if (!button) return;
  button.setAttribute("aria-pressed", String(root.classList.contains("dark")));
  button.addEventListener("click", function () {
    var dark = !root.classList.contains("dark");
    root.classList.toggle("dark", dark);
    button.setAttribute("aria-pressed", String(dark));
    try {
      localStorage.setItem("site-theme", dark ? "dark" : "light");
    } catch (e) {}
  });
})();
