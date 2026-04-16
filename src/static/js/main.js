// HTML for additional file upload element
let additionalFileHTML = `
<div class="input-base-container" id="input_name">
  <p>Additional File</p>
  <div class="additional-file-input">
    <input
      id="input_name"
      name="input_name"
      required
      type="file"
      accept=".pdf,.jpg,.jpeg,.png"
      class="input-base-item right-margin"
    />
    <span class="additional-file-input-section" id="upload_name-remove-button">
    </span>
  </div>
</div>
`;

// random number util
const getRandomNumber = (min, max) => {
  return Math.floor(Math.random() * (max - min) + min);
};

// Add file upload elements if button selected
if (
  window.location.pathname === "/" ||
  window.location.pathname === "/register-loan"
) {
  const uploadAdditionalFile = document.getElementById(
    "upload-additional-file",
  );

  uploadAdditionalFile.addEventListener("click", () => {
    let itemid = `additional_file_${getRandomNumber(1, 1000000)}`;
    let removeButtonId = `remove_additional_file_${getRandomNumber(1, 1000000)}`;

    // Create new remove button element
    let newRemoveButtonContainer = document.createElement("span");
    newRemoveButtonContainer.classList.add("additional-file-input-section");
    newRemoveButtonContainer.id = `${removeButtonId}-container`;
    let newRemoveButton = document.createElement("button");
    newRemoveButton.id = removeButtonId;
    newRemoveButton.classList.add("upload-button");
    newRemoveButton.type = "button";
    newRemoveButton.innerText = "Remove additional file";
    // Add event listener to new remove button
    newRemoveButton.addEventListener("click", () => {
      document.getElementById(itemid).remove();
    });
    newRemoveButtonContainer.appendChild(newRemoveButton);

    // Add new file input element, with remove button
    let inputBaseContainer = document.createElement("div");
    inputBaseContainer.id = itemid;
    inputBaseContainer.classList.add("input-base-container");
    inputBaseContainer.innerHTML = additionalFileHTML
      .replace(/input_name/g, itemid)
      .replace(/upload_name/g, removeButtonId);
    inputBaseContainer
      .getElementsByTagName("span")[0]
      .appendChild(newRemoveButton);

    const additionalFileContainer = document.getElementById(
      "additional-file-container",
    );
    additionalFileContainer.appendChild(inputBaseContainer);
  });
}
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
let successPopupHTML = `
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
`;

// Google Sign-In
function handleCredentialResponse(response) {
  const applicationInput = document.getElementById("application");
  const selectedLoginType = applicationInput.value; // Get the selected login type from the hidden input

  // Send the ID token to the backend for verification and authentication
  fetch("/login-google", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      token: response.credential,
      application: selectedLoginType,
    }),
  })
    .then((res) => res.json())
    .then((data) => {
      if (data.status === "OK") {
        if (selectedLoginType === "loans") {
          // Redirect to the app login url
          window.location.href = `https://app.robia.ug/login/${data.token}`;
        } else if (selectedLoginType === "pro") {
          // Redirect to the pro login url
          window.location.href = `https://pro.robia.ug/login/${data.token}`;
        }
      } else if (data.status === "MISSING") {
        // Show popup element prompting user to sign up
        successPopupHTML = successPopupHTML.replace(
          "message",
          "No account found with this email. Please sign up first.",
        );
        let newdiv = document.createElement("div");
        newdiv.innerHTML = successPopupHTML;
        document.body.appendChild(newdiv);
        // After 10 seconds, remove the popup
        setTimeout(() => {
          newdiv.remove();
        }, 9900);
      } else {
        console.error("Login error:", data.status);
      }
    })
    .catch((error) => {
      console.error("Error during login:", error);
    });
}

// Forgot password input value matching
let newPassword = document.getElementById("new-password");
let confirmPassword = document.getElementById("confirm-password");
if (newPassword && confirmPassword) {
  confirmPassword.addEventListener("input", () => {
    if (confirmPassword.value !== newPassword.value) {
      confirmPassword.setCustomValidity("Passwords do not match");
    } else {
      confirmPassword.setCustomValidity("");
    }
  });
}

let selected_application = "loans";

window.onload = function () {
  if (window.location.pathname === "/login") {
    // On select login application, set hidden input value to selected application
    let loanApplicationButton = document.getElementById("login-button-loans");
    let proApplicationButton = document.getElementById("login-button-pro");
    let applicationInput = document.getElementById("application");

    loanApplicationButton.addEventListener("click", () => {
      applicationInput.value = "loans";
      selected_application = applicationInput.value;
      loanApplicationButton.classList.add("login-button-active");
      loanApplicationButton.classList.remove("login-button-rest");
      proApplicationButton.classList.add("pro-button-rest");
      proApplicationButton.classList.remove("pro-button-active");
    });

    proApplicationButton.addEventListener("click", () => {
      applicationInput.value = "pro";
      selected_application = applicationInput.value;
      proApplicationButton.classList.add("pro-button-active");
      proApplicationButton.classList.remove("pro-button-rest");
      loanApplicationButton.classList.add("login-button-rest");
      loanApplicationButton.classList.remove("login-button-active");
    });

    // Set default selected application to loans
    applicationInput.value = "loans";
  } else if (
    window.location.pathname === "/" ||
    window.location.pathname === "/provide-loan" ||
    window.location.pathname === "/register-loan"
  ) {
    // Select default subscription plan on pricing page
    let beginnerPlanButton = document.getElementById("beginner-plan-button");
    let beginnerPlanCard = document.getElementById("beginner-plan-card");
    let proPlanButton = document.getElementById("pro-plan-button");
    let proPlanCard = document.getElementById("pro-plan-card");
    let ultimatePlanButton = document.getElementById("ultimate-plan-button");
    let ultimatePlanCard = document.getElementById("ultimate-plan-card");
    let planInput = document.getElementById("plan");

    beginnerPlanButton.addEventListener("click", () => {
      planInput.value = "beginner";
      beginnerPlanCard.classList.add("payment-card-selected");
      proPlanCard.classList.remove("payment-card-selected");
      ultimatePlanCard.classList.remove("payment-card-selected");
    });

    proPlanButton.addEventListener("click", () => {
      planInput.value = "pro";
      proPlanCard.classList.add("payment-card-selected");
      beginnerPlanCard.classList.remove("payment-card-selected");
      ultimatePlanCard.classList.remove("payment-card-selected");
    });

    ultimatePlanButton.addEventListener("click", () => {
      planInput.value = "ultimate";
      ultimatePlanCard.classList.add("payment-card-selected");
      beginnerPlanCard.classList.remove("payment-card-selected");
      proPlanCard.classList.remove("payment-card-selected");
    });

    // Set default selected plan to beginner
    planInput.value = "beginner";
  }
};
