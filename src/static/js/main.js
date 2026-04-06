// Hide success popups after 10 seconds
const successPopup = document.getElementsByClassName("success-popup")[0];
if (successPopup) {
  setTimeout(() => {
    successPopup.style.display = "none";
  }, 10000);
}

// Show form errors at the top of the page
const goToTop = document.getElementsByClassName("error-message")[0];
if (goToTop) {
  goToTop.scrollIntoView({ behavior: "instant", block: "start" });
}

// Hide error popups after 10 seconds
const errorPopup = document.getElementsByClassName("error-popup")[0];
if (errorPopup) {
  setTimeout(() => {
    errorPopup.style.display = "none";
  }, 10000);
}

// HTML for success popup
let successPopupHTML = ```
<div class="success-popup">
  <div class="success-popup-center">
    <div>
      <svg
        xmlns="http://www.w3.org/2000/svg"
        height="24px"
        viewBox="0 -960 960 960"
        width="24px"
        fill="#e3e3e3"
      >
        <path
          d="m424-296 282-282-56-56-226 226-114-114-56 56 170 170Zm56 216q-83 0-156-31.5T197-197q-54-54-85.5-127T80-480q0-83 31.5-156T197-763q54-54 127-85.5T480-880q83 0 156 31.5T763-763q54 54 85.5 127T880-480q0 83-31.5 156T763-197q-54 54-127 85.5T480-80Zm0-80q134 0 227-93t93-227q0-134-93-227t-227-93q-134 0-227 93t-93 227q0 134 93 227t227 93Zm0-320Z"
        />
      </svg>
      <p>message</p>
    </div>
    <div class="shrinking-horizontal-bar"></div>
  </div>
</div>
```;

// Google Sign-In
function handleCredentialResponse(response) {
  // Get the selected login type (loans or pro) from the UI
  const selectedLoginType = document.querySelector(
    ".login-button-group .generic-stroke",
  ).id;
  // Send the ID token to the backend for verification and authentication
  fetch("/login-google", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      token: response.credential,
      app: selectedLoginType,
    }),
  })
    .then((res) => res.json())
    .then((data) => {
      if (data.status === "OK") {
        if (selectedLoginType === "login-button-loans") {
          // Redirect to the dashboard after successful login
          window.location.href = "https://app.robia.ug/dashboard";
        } else if (selectedLoginType === "login-button-pro") {
          // Redirect to the pro dashboard after successful login
          window.location.href = "https://app.robia.ug/pro-dashboard";
        }
      } else if (data.status === "NOT_FOUND") {
        // Show popup element prompting user to sign up
        successPopupHTML = successPopupHTML.replace(
          "message",
          "No account found with this email. Please sign up first.",
        );
        let newdiv = document.createElement("div");
        newdiv.innerHTML = successPopupHTML;
        document.body.appendChild(newdiv);
        // After 10 seconds, redirect to the sign-up page
        setTimeout(() => {
          if (selectedLoginType === "login-button-loans") {
            window.location.href = `${window.location.origin}/#quick-loan`;
          } else if (selectedLoginType === "login-button-pro") {
            window.location.href = `${window.location.origin}/#pro-signup`;
          }
        }, 10000);
      } else {
        // Handle other errors (e.g., show an error message)
        console.error("Login error:", data.error);
      }
    })
    .catch((error) => {
      console.error("Error during login:", error);
    });
}

// Login page button selection
function selectButton(button) {
  const loansButton = document.querySelector(".login-button-group-loans");
  const proButton = document.querySelector(".login-button-group-pro");

  if (button === "loans") {
    // Remove styles from pro button and add to loans button
    proButton.classList.add("no-border");
    proButton.classList.add("muted-background");
    proButton.classList.remove("generic-stroke");
    // Add styles to loans button
    loansButton.classList.remove("no-border");
    loansButton.classList.remove("muted-background");
    loansButton.classList.add("generic-stroke");
  } else if (button === "pro") {
    // Remove styles from loans button and add to pro button
    loansButton.classList.add("no-border");
    loansButton.classList.add("muted-background");
    loansButton.classList.remove("generic-stroke");
    // Add styles to pro button
    proButton.classList.remove("no-border");
    proButton.classList.remove("muted-background");
    proButton.classList.add("generic-stroke");
  }
}

window.onload = () => {
  document
    .getElementById("login-button-loans")
    .addEventListener("click", () => selectButton("loans"));
  document
    .getElementById("login-button-pro")
    .addEventListener("click", () => selectButton("pro"));
};
