import React from "react"
import Box from "@mui/material/Box"
import { Router } from "../routing/router"
import ErrorBoundary from "../../components/common/ErrorBoundary"
import { paperbaseTheme } from "./paperbase-theme"

export const BaseContent = () => {
  return (
    <React.Fragment>
      <Box
        sx={{
          py: 6,
          px: 4,
          flex: 1,
          height: "90vh",
          bgcolor: paperbaseTheme.palette.primary.dark,
          border: "1px solid charcoal",
        }}
      >
        <ErrorBoundary>
          <Router />
        </ErrorBoundary>
      </Box>
    </React.Fragment>
  )
}
