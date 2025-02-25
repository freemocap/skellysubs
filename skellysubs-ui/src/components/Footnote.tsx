import Typography from "@mui/material/Typography"
import Link from "@mui/material/Link"
import * as React from "react"

export const Footnote = function () {
  return (
    <Typography variant="body2" color="#777" align="center">
      {"w/ "}
      <Link color="inherit" href="https://github.com/freemocap/">
        ❤️
      </Link>
      {"  from the "}
      <Link color="inherit" href="https://github.com/freemocap/">
        FreeMoCap Foundation
      </Link>{" "}
      {new Date().getFullYear()}
      <Link color="inherit" href="https://github.com/freemocap/skellysubs">
        [source code]
      </Link>
    </Typography>
  )
}
