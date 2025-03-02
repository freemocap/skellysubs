import * as React from "react"
import { ThemeProvider } from "@mui/material/styles"
import { HashRouter } from "react-router-dom"
import { CssBaseline } from "@mui/material"
import extendedPaperbaseTheme from "./paperbase-theme"
import { BasePanelLayout } from "../BasePanelLayout"
import { MainContent } from "../MainContent"

export const PaperbaseContent = function () {
  return (
    <ThemeProvider theme={extendedPaperbaseTheme}>
      <CssBaseline />
      <HashRouter>
        <BasePanelLayout>
          <MainContent />
        </BasePanelLayout>
      </HashRouter>
    </ThemeProvider>
  )
}
