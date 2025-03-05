// skellysubs-ui/src/store/slices/processing-configs/translationConfigSlice.ts
import type { PayloadAction } from "@reduxjs/toolkit"
import { createAsyncThunk, createSlice } from "@reduxjs/toolkit"
import type { LanguageConfig } from "../../../schemas/languageConfigSchemas"
import type { RootState } from "../../AppStateStore"
import { getLanguageConfigs } from "../../../utils/getLanguageConfigs"
import { logger } from "../../../utils/logger"

interface TranslationConfigState {
  availableTargetLanguages: Record<string, LanguageConfig>
  selectedTargetLanguages: string[]
}

const initialState: TranslationConfigState = {
  availableTargetLanguages: {} as Record<string, LanguageConfig>,
  selectedTargetLanguages: [],
}
export const fetchLanguageConfigs = createAsyncThunk(
  "translationConfig/fetchLanguageConfigs",
  async () => {
    logger(
      "[TranslationConfigSlice - fetchLanguageConfigs] Loading language configs...",
    )
    try{return await getLanguageConfigs()} catch (error) {
        logger(
            `[TranslationConfigSlice - fetchLanguageConfigs] Error loading language configs: ${error}`,
        )
        throw error
    }

  },
)

export const translationConfigSlice = createSlice({
  name: "translationConfig",
  initialState,
  reducers: {
    toggleLanguage: (state, action: PayloadAction<string>) => {
      const code = action.payload
      const index = state.selectedTargetLanguages.indexOf(code)
      if (index === -1) {
        state.selectedTargetLanguages.push(code)
      } else {
        state.selectedTargetLanguages.splice(index, 1)
      }
    },
    addCustomLanguage: (state, action: PayloadAction<LanguageConfig>) => {
      const lang = action.payload
      const key = lang.language_code // Use code instead of name
      state.availableTargetLanguages[key] = lang
    },
    updateLanguageConfig: (
      state,
      action: PayloadAction<{ key: string; config: LanguageConfig }>,
    ) => {
      const { key, config } = action.payload
      state.availableTargetLanguages[key] = config
    },
  },
  extraReducers: builder => {
    builder.addCase(fetchLanguageConfigs.fulfilled, (state, action) => {
      logger(
        `[TranslationConfigSlice - fetchLanguageConfigs] Fulfilled - available languages: ${Object.keys(action.payload).join(", ")}`,
      )
      state.availableTargetLanguages = action.payload
    })
  },
})

export const { toggleLanguage, addCustomLanguage, updateLanguageConfig } =
  translationConfigSlice.actions

export const selectLanguageOptions = (state: RootState) =>
  state.translationConfig.availableTargetLanguages

export const selectSelectedTargetLanguages = (state: RootState) =>
  state.translationConfig.selectedTargetLanguages
