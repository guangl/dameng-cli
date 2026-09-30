# Runtime completion includes installed plugins and saved connection names.
_dm() {
    local value i
    local -a args=()
    for ((i=1; i<=COMP_CWORD; i++)); do
        if [[ ${COMP_WORDS[i]} == = ]]; then
            (( i == COMP_CWORD )) && args+=("")
        else
            args+=("${COMP_WORDS[i]}")
        fi
    done
    COMPREPLY=()
    while IFS= read -r value; do
        COMPREPLY+=("$value")
    done < <(dm complete -- "${args[@]}" 2>/dev/null)
}
complete -o filenames -F _dm dm
