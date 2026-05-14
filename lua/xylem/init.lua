local M = {}

local uv = vim.uv
local job_id = nil
local stdout_buffer = ""

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
        rpc = false, -- We handle our own RPC format
        stdout_buffered = false,
        stderr = "pipe",

        on_stdout = function(_, data)
            if data then
                stdout_buffer = stdout_buffer .. table.concat(data, "\n")
                while true do
                    local header_end = stdout_buffer:find("\r\n\r\n")
                    if not header_end then break end
                    
                    local header = stdout_buffer:sub(1, header_end - 1)
                    local content_length = header:match("Content%-Length: (%d+)")
                    
                    if not content_length then
                        -- Try to find next header or just clear if malformed
                        stdout_buffer = stdout_buffer:sub(header_end + 4)
                    else
                        content_length = tonumber(content_length)
                        if #stdout_buffer < header_end + 3 + content_length then
                            break
                        end
                        
                        local body = stdout_buffer:sub(header_end + 4, header_end + 3 + content_length)
                        stdout_buffer = stdout_buffer:sub(header_end + 4 + content_length)
                        
                        local ok, decoded = pcall(vim.json.decode, body)
                        if ok then
                            M.handle_message(decoded)
                        else
                            vim.notify("[xylem] Failed to decode JSON: " .. body, vim.log.levels.ERROR)
                        end
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
            if code ~= 0 then
                vim.notify("[xylem] Process exited with code " .. code, vim.log.levels.ERROR)
            end
        end,
    })

    if job_id > 0 then
        vim.notify("[xylem] Started successfully", vim.log.levels.INFO)
        M.setup_autocmds()
    else
        vim.notify("[xylem] Failed to start", vim.log.levels.ERROR)
    end
end

function M.handle_message(msg)
    if msg.method == "xylem.highlights" then
        M.apply_highlights(msg.params.buffer_id, msg.params.highlights)
    end
end

function M.send_message(msg)
    if not job_id or job_id <= 0 then
        return
    end

    local encoded = vim.json.encode(msg)
    local header = string.format("Content-Length: %d\r\n\r\n", #encoded)
    vim.fn.chansend(job_id, header .. encoded)
end

function M.setup_autocmds()
    local group = vim.api.nvim_create_augroup("xylem", { clear = true })

    vim.api.nvim_create_autocmd({ "TextChanged", "TextChangedI" }, {
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
    M.send_message({
        method = "xylem.attach",
        params = { buffer_id = buf_id },
    })
end

function M.detach_buffer(buf_id)
    M.send_message({
        method = "xylem.detach",
        params = { buffer_id = buf_id },
    })
end

function M.notify_change(buf_id, text)
    M.send_message({
        method = "xylem.change",
        params = {
            buffer_id = buf_id,
            text = text,
        },
    })
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
