package app.coaxi.android.feature.settings

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.coaxi.android.core.datastore.AppSettings
import app.coaxi.android.core.datastore.AppSettingsStore
import app.coaxi.android.core.datastore.ThemeMode
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class CoaxiAppearanceViewModel @Inject constructor(private val store:AppSettingsStore):ViewModel(){
    val settings=store.settings.stateIn(viewModelScope,SharingStarted.Eagerly,AppSettings())
    fun theme(mode:ThemeMode){viewModelScope.launch{store.setThemeMode(mode)}}
    fun accent(id:String){viewModelScope.launch{store.setAccent(id)}}
}
@Composable
fun CoaxiAppearanceSettings(model:CoaxiAppearanceViewModel=hiltViewModel()){
    val settings by model.settings.collectAsStateWithLifecycle()
    AppearanceSettingsContent(settings,model::theme,model::accent)
}
