// Image refresh
var last_refresh_image = 0;
var mediasession_enabled = false;
function refresh_image() {
  $("img").each(function () {
    $(this).attr("src", $(this).attr("src") + "?" + Math.random());
    last_refresh_image = Date.now();
  });
}
// Times and progress bar refresh
function refresh_times() {
  $.get("/times", {}, function (data) {
    $("#current").html(data.current);
    $("#total").html(data.total);
    $("#position").css("width", data.perc + "%");
    if ("mediaSession" in navigator) {
      $("#audio")[0].currentTime = data.current_s;
      navigator.mediaSession.setPositionState({
        duration: data.total_s,
        position: data.current_s,
      });
    }
  });
}

$(document).ready(function () {
  refresh_times();
  // Refresh timers
  setInterval(function () {
    refresh_times();
  }, 1000);
  setInterval(function () {
    if (Date.now() - last_refresh_image > 3000) {
      refresh_image();
    }
  }, 1000);

  // Seeking by clicking on the progress bar
  $("#bar").click(function (e) {
    enable_mediasession();
    var perc = (100.0 * (e.pageX - $(this).parent().offset().left)) / $("#bar").width();
    $.get("/action/seek", { position: perc }, function () {
      refresh_image();
    });
  });
  // Clicking on the action buttons
  $(".action").click(function () {
    enable_mediasession();
    action(this.id);
  });
});

function action(id) {
  $.get("/action/" + id, {}, function () {
    refresh_times();
    refresh_image();
    if ("mediaSession" in navigator) {
      if (id == "play") {
        navigator.mediaSession.playbackState = "playing";
        audio.play();
      } else if (id == "pause") {
        navigator.mediaSession.playbackState = "paused";
        audio.pause();
      }
    }
  });
}

function enable_mediasession() {
  if (mediasession_enabled) {
    return;
  }
  var audio = $("#audio")[0];
  audio.play();
  if ("mediaSession" in navigator) {
    navigator.mediaSession.metadata = new MediaMetadata({
      title: "mpv",
      artwork: [{ src: "/screenshot", sizes: "300x300", type: "image/jpg" }],
    });
    navigator.mediaSession.setActionHandler("play", () => {
      audio.play();
      action("play");
    });
    navigator.mediaSession.setActionHandler("pause", () => {
      audio.pause();
      action("pause");
    });
    navigator.mediaSession.setActionHandler("seekbackward", (details) => action("rewind"));
    navigator.mediaSession.setActionHandler("previoustrack", () => action("rewind"));
  }
  mediasession_enabled = true;
}
