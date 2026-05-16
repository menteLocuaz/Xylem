local M = {}
local job_id = nil
local attached = {}
local ns = vim.api.nvim_create_namespace("xylem")

function M.setup()
    if job_id then return end
    -- 1. Conectar runtime
    job_id = vim.fn.jobstart({ "xylem", "--rpc" }, {
        rpc = true,
        on_exit = function()
            job_id = nil
            attached = {}
        end
    })

    -- 2. Registrar autocmds
    local group = vim.api.nvim_create_augroup("xylem", { clear = true })
    vim.api.nvim_create_autocmd("BufEnter", {
        group = group,
        callback = function(ev)
            M.attach(ev.buf)
        end,
    })
end

function M.attach(buf)
    -- Guardas de seguridad y duplicación
    if not job_id or job_id <= 0 then return end
    if attached[buf] or vim.bo[buf].buftype ~= "" then return end

    attached[buf] = true
    vim.rpcnotify(job_id, "xylem.attach", { buffer_id = buf })

    -- 3. Enviar eventos
    vim.api.nvim_buf_attach(buf, false, {
        on_bytes = function(_, b, _, s_row, s_col, s_byte, _, _, o_byte, n_row, n_col, _)
            -- Diferir la captura para no bloquear el event loop principal
            vim.schedule(function()
                if not vim.api.nvim_buf_is_valid(b) then return end

                -- Cálculo robusto de coordenadas finales para nvim_buf_get_text
                local e_row = s_row + n_row
                local e_col = n_row == 0 and (s_col + n_col) or n_col

                -- Captura quirúrgica del texto nuevo
                local text = table.concat(vim.api.nvim_buf_get_text(b, s_row, s_col, e_row, e_col, {}), "\n")

                vim.rpcnotify(job_id, "xylem.change", {
                    buffer_id = b,
                    start_byte = s_byte,
                    old_end_byte = s_byte + o_byte,
                    new_text = text,
                })
            end)
        end,
        on_detach = function(_, b)
            attached[b] = nil
            if job_id and job_id > 0 then
                vim.rpcnotify(job_id, "xylem.detach", { buffer_id = b })
            end
        end,
    })
end

function M.apply_highlight_delta(params)
    local buf = params.buffer_id
    if not vim.api.nvim_buf_is_valid(buf) then return end

    for _, delta in ipairs(params.deltas) do
        -- Uso eficiente del namespace a nivel de módulo
        vim.api.nvim_buf_clear_namespace(buf, ns, delta.line, delta.line + 1)
        for _, cap in ipairs(delta.captures) do
            vim.api.nvim_buf_add_highlight(buf, ns, cap.hl_group, delta.line, cap.start_col, cap.end_col)
        end
    end
end

return M
