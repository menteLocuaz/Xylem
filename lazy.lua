{
    "~/projects/xylem",
    lazy = false,
    version = false,
    build = "cargo build --release",
    config = function()
        require("xylem").start()
    end,
}