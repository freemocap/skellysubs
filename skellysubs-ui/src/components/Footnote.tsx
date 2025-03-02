import Typography from "@mui/material/Typography"
import Link from "@mui/material/Link"
import * as React from "react"

export const Footnote = function () {
  return (
    <Typography variant="body2" color="#777" align="center">
      {"w/ "}
      <Link
        color="inherit"
        href="https://freemocap.org/about-us.html#donate"
        target="_blank"
        rel="noopener noreferrer"
      >
        ❤️
      </Link>
      {"  from the "}
      <Link
        color="inherit"
        href="https://github.com/freemocap/"
        target="_blank"
        rel="noopener noreferrer"
      >
        FreeMoCap Foundation
      </Link>{" "}
      {new Date().getFullYear()}
      <br />
      <br />
      <Link
        color="inherit"
        href="https://github.com/freemocap/skellysubs"
        target="_blank"
        rel="noopener noreferrer"
      >
        [source code - https://github.com/freemocap/skellysubs]
      </Link>
    </Typography>
  )
}
