local M = {}

local uv = vim.uv
local job_id = nil
local rpc_channel = nil
local event_handlers = {}

function M.start()
    if job_id then
        vim.notify("[xylem] Already running", vim.log.levels.WARN)
        return
    end

    local binary_path = vim.fn.stdpath("data") .. "/xylem/target/release/xylem"

    if not vim.fn.executable(binary_path) then
        binary_path = "./target/release/xylem"
    end

    job_id = vim.fn.jobstart({
        binary_path,
        "--rpc",
    }, {
        rpc = true,
        stdout_buffered = false,
        stderr = "pipe",

        on_stdout = function(_, data)
            if data then
                for _, line in ipairs(data) do
                    if line ~= "" then
                        vim.notify("[xylem] " .. line, vim.log.levels.INFO)
                    end
                end
            end
        end,

        on_stderr = function(_, data)
            if data then
                for _, line in ipairs(data) do
                    if line ~= "" then
                        vim.notify("[xylem] ERROR: " .. line, vim.log.levels.ERROR)
                    end
                end
            end
        end,

        on_exit = function(_, code)
            job_id = nil
            rpc_channel = nil
            if code ~= 0 then
                vim.notify("[xylem] Process exited with code " .. code, vim.log.levels.ERROR)
            end
        end,
    })

    if job_id > 0 then
        rpc_channel = vim.fn.jobstart({
            binary_path,
        }, {
            rpc = true,
        })
        vim.notify("[xylem] Started successfully", vim.log.levels.INFO)
        M.setup_autocmds()
    else
        vim.notify("[xylem] Failed to start", vim.log.levels.ERROR)
    end
end

function M.setup_autocmds()
    local group = vim.api.nvim_create_augroup("xylem", { clear = true })

    vim.api.nvim_create_autocmd("TextChangedI", {
        group = group,
        pattern = "*.lua",
        callback = function(args)
            local buf_id = args.buf
            local text = vim.api.nvim_buf_get_lines(buf_id, 0, -1, false)
            M.notify_change(buf_id, table.concat(text, "\n"))
        end,
    })

    vim.api.nvim_create_autocmd("TextChanged", {
        group = group,
        pattern = "*.lua",
        callback = function(args)
            local buf_id = args.buf
            local text = vim.api.nvim_buf_get_lines(buf_id, 0, -1, false)
            M.notify_change(buf_id, table.concat(text, "\n"))
        end,
    })

    vim.api.nvim_create_autocmd("BufEnter", {
        group = group,
        pattern = "*.lua",
        callback = function(args)
            M.attach_buffer(args.buf)
        end,
    })

    vim.api.nvim_create_autocmd("BufLeave", {
        group = group,
        pattern = "*.lua",
        callback = function(args)
            M.detach_buffer(args.buf)
        end,
    })
end

function M.attach_buffer(buf_id)
    if not job_id or job_id <= 0 then
        return
    end

    vim.fn.chansend(job_id, vim.json.encode({
        method = "xylem.attach",
        params = { buffer_id = buf_id },
    }) .. "\n")
end

function M.detach_buffer(buf_id)
    if not job_id or job_id <= 0 then
        return
    end

    vim.fn.chansend(job_id, vim.json.encode({
        method = "xylem.detach",
        params = { buffer_id = buf_id },
    }) .. "\n")
end

function M.notify_change(buf_id, text)
    if not job_id or job_id <= 0 then
        return
    end

    vim.fn.chansend(job_id, vim.json.encode({
        method = "xylem.change",
        params = {
            buffer_id = buf_id,
            text = text,
        },
    }) .. "\n")
end

function M.apply_highlights(buf_id, highlights)
    if not vim.api.nvim_buf_is_loaded(buf_id) then
        return
    end

    vim.highlight.clear(buf_id, "xylem")
    for _, hl in ipairs(highlights) do
        local start_pos = M.byte_to_pos(buf_id, hl.start_byte)
        local end_pos = M.byte_to_pos(buf_id, hl.end_byte)
        vim.highlight.range(
            buf_id,
            "xylem",
            hl.hl_group,
            start_pos,
            end_pos,
            {}
        )
    end
end

function M.byte_to_pos(buf_id, byte)
    local text = vim.api.nvim_buf_get_lines(buf_id, 0, -1, false)
    local pos = { 0, 0 }
    local current_byte = 0

    for i, line in ipairs(text) do
        local line_bytes = #line + 1
        if current_byte + line_bytes > byte then
            pos = { i - 1, byte - current_byte }
            break
        end
        current_byte = current_byte + line_bytes
    end

    return pos
end

function M.stop()
    if job_id and job_id > 0 then
        vim.fn.jobstop(job_id)
        job_id = nil
        rpc_channel = nil
        vim.notify("[xylem] Stopped", vim.log.levels.INFO)
    end
end

function M.is_running()
    return job_id ~= nil and job_id > 0
end

function M.get_status()
    return {
        running = M.is_running(),
        job_id = job_id,
    }
end

return M