// Image refresh
var last_refresh_image = 0;
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
    var perc = (100.0 * (e.pageX - $(this).parent().offset().left)) / $("#bar").width();
    $.get("/action/seek", { position: perc }, function () {
      refresh_image();
    });
  });
  // Clicking on the action buttons
  $(".action").click(function () {
    action(this.id);
  });
});

function action(id) {
  $.get("/action/" + id, {}, function () {
    refresh_times();
    refresh_image();
  });
}
